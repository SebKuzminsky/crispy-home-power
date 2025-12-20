//! The starter code slowly blinks the LED and sets up
//! USB logging. It periodically logs messages over USB.

#![no_std]
#![no_main]

use teensy4_panic as _;

#[rtic::app(device = teensy4_bsp, peripherals = true, dispatchers = [KPP])]
mod app {
    use bsp::board;
    use teensy4_bsp as bsp;

    use imxrt_log as logging;

    // If you're using a Teensy 4.1 or MicroMod, you should eventually
    // change 't40' to 't41' or micromod, respectively.
    use board::t40 as my_board;

    use rtic_monotonics::systick::{Systick, *};

    /// There are no resources shared across tasks.
    #[shared]
    struct Shared {}

    /// These resources are local to individual tasks.
    #[local]
    struct Local {
        /// The LED on pin 13.
        led: board::Led,
        /// A poller to control USB logging.
        poller: logging::Poller,
        /// The CAN1 interface on pins 22 (Tx) and 23 (Rx)
        can1: board::Flexcan1,
    }

    #[init]
    fn init(cx: init::Context) -> (Shared, Local) {
        let board::Resources {
            mut gpio2,
            pins,
            usb,
            flexcan1,
            ..
        } = my_board(cx.device);

        let led = board::led(&mut gpio2, pins.p13);
        let poller = logging::log::usbd(usb, logging::Interrupts::Enabled).unwrap();

        let mut can1 = board::flexcan(flexcan1, pins.p22, pins.p23);
        can1.set_baud_rate(500_000);
        can1.set_max_mailbox(16);
        can1.disable_fifo();

        Systick::start(
            cx.core.SYST,
            board::ARM_FREQUENCY,
            rtic_monotonics::create_systick_token!(),
        );

        blink::spawn().unwrap();
        battery_task::spawn().unwrap();

        (Shared {}, Local { led, poller, can1 })
    }

    #[task(local = [led])]
    async fn blink(cx: blink::Context) {
        let mut count = 0u32;
        loop {
            cx.local.led.toggle();
            Systick::delay(500.millis()).await;

            log::info!("Hello from your Teensy 4! The count is {count}");
            if count % 7 == 0 {
                log::warn!("Here's a warning at count {count}");
            }
            if count % 23 == 0 {
                log::error!("Here's an error at count {count}");
            }

            count = count.wrapping_add(1);
        }
    }

    #[task(local = [can1])]
    async fn battery_task(context: battery_task::Context) {
        let can = context.local.can1;

        loop {
            // read all available mailboxes for any available frames
            if let Some(data) = can.read_mailboxes() {
                // TODO: get rid of this and make it a "real" message
                let frame_id = data.frame.id();
                let id = match frame_id {
                    imxrt_hal::can::Id::Standard(v) => v.as_raw().into(),
                    imxrt_hal::can::Id::Extended(v) => v.as_raw(),
                };

                // Decode the CAN message
                if let Some(payload) = data.frame.data() {
                    log::info!("{:?} {:?}", id, payload);
                }
            }

            Systick::delay(5.millis()).await;
        }
    }

    #[task(binds = USB_OTG1, local = [poller])]
    fn log_over_usb(cx: log_over_usb::Context) {
        cx.local.poller.poll();
    }
}
