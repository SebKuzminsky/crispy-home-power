//! The starter code slowly blinks the LED and sets up
//! USB logging. It periodically logs messages over USB.

#![no_std]
#![no_main]

mod abs_alliance_can_messages;
mod debounced_input_pin;
mod delta_q_can_messages;

use teensy4_panic as _;

#[derive(Clone, Debug, PartialEq)]
pub enum ControllerMode {
    /// The controller is booting, wait for the user to release the power button then go to
    /// Discharge mode.
    Booting,

    /// HV bus is on and providing power.
    Discharge,

    /// User has requested the battery turn off (de-energize HV bus).
    Sleep,
}

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {
    use bsp::board;
    use teensy4_bsp as bsp;

    use imxrt_log as logging;

    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;

    use rtic_monotonics::systick::{Systick, *};

    use embedded_can::Frame;
    use embedded_hal::digital::InputPin;

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {
        /// The CAN1 interface on pins 22 (Tx) and 23 (Rx), connected
        /// to the ABS Alliance battery pack.
        can1: board::Flexcan1,

        /// The CAN2 interface on pins 1 (Tx) and 0 (Rx), connected to
        /// the Delta-Q charger.
        can2: board::Flexcan2,

        /// The current mode.
        controller_mode: crate::ControllerMode,
    }

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        led: board::Led,

        /// Power button, pulls down when pressed.
        power_button_gpio: crate::debounced_input_pin::DebouncedInputPin<
            teensy4_bsp::hal::gpio::Input<teensy4_bsp::pins::t40::P2>,
        >,

        /// A poller to control USB logging.
        poller: logging::Poller,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            mut gpio4,
            mut pins,
            usb,
            flexcan1,
            flexcan2,
            ..
        } = my_board(cx.device);

        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();

        let mut can1 = board::flexcan(flexcan1, pins.p22, pins.p23);
        can1.set_baud_rate(500_000);
        can1.set_max_mailbox(16);
        can1.disable_fifo();

        let mut can2 = board::flexcan(flexcan2, pins.p1, pins.p0);
        can2.set_baud_rate(500_000);
        can2.set_max_mailbox(16);
        can2.disable_fifo();

        const PIN_CONFIG: bsp::hal::iomuxc::Config = bsp::hal::iomuxc::Config::zero()
            .set_pull_keeper(Some(bsp::hal::iomuxc::PullKeeper::Pullup100k));
        bsp::hal::iomuxc::configure(&mut pins.p2, PIN_CONFIG);
        let power_button_gpio =
            crate::debounced_input_pin::DebouncedInputPin::new(gpio4.input(pins.p2), 10);

        let controller_mode = crate::ControllerMode::Booting;

        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        blink::spawn().unwrap();
        battery_can_rx_task::spawn().unwrap();
        battery_can_tx_task::spawn().unwrap();
        charger_task::spawn().unwrap();
        power_button_task::spawn().unwrap();

        (
            Shared {
                can1,
                can2,
                controller_mode,
            },
            Local {
                led,
                power_button_gpio,
                poller,
            },
        )
    }

    #[task(local = [led], shared = [controller_mode])]
    async fn blink(context: blink::Context) {
        let led = context.local.led;
        let mut controller_mode = context.shared.controller_mode;
        loop {
            led.toggle();

            let delay_ms = match controller_mode.lock(|mode| mode.clone()) {
                crate::ControllerMode::Booting => 100,
                crate::ControllerMode::Discharge => 250,
                crate::ControllerMode::Sleep => 500,
            };
            Systick::delay(delay_ms.millis()).await;
        }
    }

    #[task(local = [power_button_gpio], shared = [controller_mode])]
    async fn power_button_task(context: power_button_task::Context) {
        let power_button_gpio = context.local.power_button_gpio;
        let mut controller_mode = context.shared.controller_mode;
        let mut prev_button_state = power_button_gpio.is_high().unwrap();

        loop {
            let button_state = power_button_gpio.is_high().unwrap();
            let button_was_pressed = prev_button_state && !button_state;
            let button_was_released = !prev_button_state && button_state;

            controller_mode.lock(|mode| {
                match mode {
                    crate::ControllerMode::Booting => {
                        // The Booting state is the only one that cares
                        // about the rising edge of the gpio (the release
                        // of the button).
                        if button_was_released {
                            *mode = crate::ControllerMode::Discharge;
                        }
                    }
                    crate::ControllerMode::Discharge => {
                        if button_was_pressed {
                            *mode = crate::ControllerMode::Sleep;
                        }
                    }
                    crate::ControllerMode::Sleep => {
                        if button_was_pressed {
                            *mode = crate::ControllerMode::Discharge;
                        }
                    }
                }
            });

            prev_button_state = button_state;

            // FIXME: lame polling loop, switch to interrupts
            Systick::delay(10.millis()).await;
        }
    }

    #[task(shared = [can1])]
    async fn battery_can_rx_task(context: battery_can_rx_task::Context) {
        let mut can = context.shared.can1;

        loop {
            // Process any incoming CAN packets that have arrived.
            match can.lock(|can| can.read_mailboxes()) {
                Some(data) => {
                    // TODO: get rid of this and make it a "real" message
                    let frame_id = data.frame.id();
                    let id = match frame_id {
                        imxrt_hal::can::Id::Standard(v) => v.as_raw().into(),
                        imxrt_hal::can::Id::Extended(v) => v.as_raw(),
                    };
                    if id == 292 || id == 261 {
                        // Decode the CAN message
                        if let Some(payload) = data.frame.data() {
                            match crate::abs_alliance_can_messages::Messages::from_can_message(
                                data.frame.id(),
                                payload,
                            ) {
                                Ok(msg) => {
                                    // log::info!("{:#?}", msg);
                                    match msg {
                                        crate::abs_alliance_can_messages::Messages::BattPackSoc(m) => {
                                            log::info!("SoC={:?}%", m.batt_pack_user_soc());
                                        }
                                        crate::abs_alliance_can_messages::Messages::BattPackHvStatus(m) => {
                                            log::info!("Vpack={:?} V", m.batt_v_pack());
                                            log::info!("Ipack={:?} A", m.batt_i_pack_filtered());
                                        }
                                        _ => {
                                            // unknown/unhandled message
                                        }
                                    }
                                }
                                Err(e) => log::error!("{e:?}"),
                            }
                        }
                    }
                }
                None => {
                    // No CAN packets to read, sleep and poll again.
                    // FIXME: switch to nonblocking interrupt-driven mode
                    Systick::delay(5.millis()).await;
                }
            }
        }
    }

    fn battery_send_host_state_request<C>(
        can: &mut C,
        state: crate::abs_alliance_can_messages::HostBatteryRequestHostStateRequest,
    ) where
        C: embedded_can::nb::Can,
    {
        let host_battery_request = crate::abs_alliance_can_messages::HostBatteryRequest::new(
            false,
            false,
            false,
            false,
            false,
            state.into(),
        )
        .unwrap();
        log::info!("sending {:#?}", host_battery_request);

        match &embedded_can::Frame::new(host_battery_request.id(), host_battery_request.data()) {
            Some(frame) => match can.transmit(frame) {
                Ok(_) => (),
                Err(e) => {
                    log::error!("error sending CAN Frame: {:#?}", e);
                }
            },
            None => {
                log::error!("error making CAN Frame from {:#?}", host_battery_request);
            }
        }
    }

    #[task(shared = [can1, controller_mode])]
    async fn battery_can_tx_task(context: battery_can_tx_task::Context) {
        let mut can = context.shared.can1;
        let mut controller_mode = context.shared.controller_mode;

        let mut battery_is_sleeping = false;

        // During normal operation: Once per second, send the "Drive" command to the battery pack.
        loop {
            let mode = controller_mode.lock(|mode| mode.clone());
            match mode {
                crate::ControllerMode::Booting => {
                    battery_is_sleeping = false;
                    can.lock(|can| battery_send_host_state_request(can, crate::abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Drive));
                }
                crate::ControllerMode::Discharge => {
                    battery_is_sleeping = false;
                    can.lock(|can| battery_send_host_state_request(can, crate::abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Drive));
                }
                crate::ControllerMode::Sleep => {
                    // The ABS Alliance batteries only tolerate one
                    // Sleep command, a second packet wakes them back up.
                    if !battery_is_sleeping {
                        can.lock(|can| battery_send_host_state_request(can, crate::abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Sleep));
                        battery_is_sleeping = true;
                        log::info!("putting battery to sleep");
                    }
                }
            };


            Systick::delay(1000.millis()).await;
        }
    }

    #[task(shared = [can2])]
    async fn charger_task(context: charger_task::Context) {
        let mut can = context.shared.can2;

        loop {
            // Process any incoming CAN packets that have arrived.
            match can.lock(|can| can.read_mailboxes()) {
                Some(data) => {
                    // TODO: get rid of this and make it a "real" message
                    let frame_id = data.frame.id();
                    let id = match frame_id {
                        imxrt_hal::can::Id::Standard(v) => v.as_raw().into(),
                        imxrt_hal::can::Id::Extended(v) => v.as_raw(),
                    };

                    // Decode the CAN message
                    if let Some(payload) = data.frame.data() {
                        match crate::delta_q_can_messages::Messages::from_can_message(
                            data.frame.id(),
                            payload,
                        ) {
                            Ok(msg) => {
                                log::info!("{:#?}", msg);
                            }
                            Err(e) => log::error!("{e:?}"),
                        }
                    }
                }
                None => {
                    // No CAN packets to read, sleep and poll again.
                    // FIXME: switch to nonblocking interrupt-driven mode
                    Systick::delay(5.millis()).await;
                }
            }
        }
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
