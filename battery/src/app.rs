use embedded_can::Frame;
use futures::future::FusedFuture;
use futures::FutureExt;
use futures_util::stream::StreamExt;

use ratatui::style::Stylize;

use battery::abs_alliance_can_messages;
use battery::tui;

// BATT_boardADC_1_5(
//     BATT_boardADC_PACK_5: 45.59 V,
//     BATT_boardADC_COMMON_DRAIN_5: 45.62 V,
//     BATT_boardADC_LOAD_5: 45.57 V
// )
#[derive(Clone, Copy, Debug, Default)]
struct Adc1 {
    pack_voltage: f32,
    common_drain_voltage: f32,
    load_voltage: f32,
}

// BATT_boardADC_2_5(
//     BATT_boardADC_5V_5: 4.97 V,
//     BATT_boardADC_12V_5: 12.47 V,
//     BATT_boardADC_3V3_5: 3.3000000000000003 V
// )
#[derive(Clone, Copy, Debug, Default)]
struct Adc2 {
    rail_5v: f32,
    rail_12v: f32,
    rail_3v3: f32,
}

// BATT_modHvStatus_0(
//     BATT_iModFiltered_0: 0.015 A,
//     BATT_iMod_0: 0.085 A,
//     BATT_vMod_0: 47.85 V,
//     BATT_vModSumOfCells_0: 47.83 V
// )
#[derive(Clone, Copy, Debug, Default)]
struct HvStatus {
    voltage: f32,
    current: f32,
}

// BATT_modChgLimits_0(
//     BATT_iModChgLimit10s_0: 50.4 A,
//     BATT_iModChgLimitCont_0: 12.0 A,
//     BATT_iModChgLimitInst_0: 64.8 A,
//     BATT_vModChgLimit_0: 52.835 V
// )
#[derive(Clone, Copy, Debug, Default)]
struct ChargeLimit {
    voltage: f32,
    current: f32,
}

// BATT_modSOC_5(
//     BATT_sohcMod_5: 91.0 %,
//     BATT_socMod_5: 8.1 %,
//     BATT_socLimiting_5: 8.75 %,
//     BATT_idSOCLimiting_5: 6
// )
#[derive(Clone, Copy, Debug, Default)]
struct SOC {
    soc: f32,
    soh: f32,
}

// BATT_modTemperaturesA_5(
//     BATT_tAmbient_5: 25.900000000000002 deg C,
//     BATT_tModule1_5: 23.8 deg C,
//     BATT_tModule2_5: 25.8 deg C
// )
#[derive(Clone, Copy, Debug, Default)]
struct TemperaturesA {
    ambient: f32,
    module1: f32,
    module2: f32,
}

// BATT_modTemperaturesB_0(
//     BATT_tFET_0: 22.900000000000002 deg C,
//     BATT_tShunt_0: 23.6 deg C
// )
#[derive(Clone, Copy, Debug, Default)]
struct TemperaturesB {
    fet: f32,
    shunt: f32,
}

#[derive(Clone, Copy, Debug, Default)]
struct BatteryModule {
    serial_number: u64,

    adc1: Adc1,
    adc2: Adc2,
    hv_status: HvStatus,
    charge_limit: ChargeLimit,
    soc: SOC,
    temperatures_a: TemperaturesA,
    temperatures_b: TemperaturesB,
    v_bricks: [f32; 14],
    balancing: [bool; 14],

    last_seen: Option<std::time::Instant>,
}

#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
struct ChargeRequest {
    voltage: f32,
    current: f32,
    enable: bool,
    last_seen: std::time::Instant,
}

// BATT_packDiagnosticConnect(
//     BATT_packModChgEntryConnOK_M: 3,
//     BATT_packModDrvEntryConnOK_M: 3,
//     BATT_packModChgStandbyConnOK_M: 0,
//     BATT_packModDrvStandbyConnOK_M: 0
// )
#[derive(Clone, Copy, Debug, Default)]
struct PackDiagnosticConnect {
    num_modules_connected_for_charge: u16,
    num_modules_connected_for_drive: u16,
    num_modules_standby_for_charge: u16,
    num_modules_standby_for_drive: u16,
}

// BATT_packHvStatus(
//     BATT_vPackLoad: 49.83 V,
//     BATT_iPack: 8.2,
//     BATT_vPack: 48.96 V,
//     BATT_iPackFiltered: 8.2 A
// )
#[derive(Clone, Copy, Debug, Default)]
struct PackHvStatus {
    voltage: f32,
    current: f32,
}

// BATT_packSOC(
//     BATT_packUserSOC: 20.1 %,
//     BATT_packSOC: 22.5 %
// )
#[derive(Clone, Copy, Debug, Default)]
struct PackSOC {
    soc: f32,
    user_soc: f32,
}

#[derive(Clone, Copy, Debug)]
struct BatteryPack {
    mode: abs_alliance_can_messages::HostBatteryRequestHostStateRequest,
    modules: [BatteryModule; 10],
    charge_request: Option<ChargeRequest>,
    pack_diagnostic_connect: PackDiagnosticConnect,
    pack_hv_status: PackHvStatus,
    pack_soc: PackSOC,
}

// Have to impl this by hand because
// abs_alliance_can_messages::HostBatteryRequestHostStateRequest doesn't
// impl default.
impl Default for BatteryPack {
    fn default() -> Self {
        BatteryPack {
            mode: abs_alliance_can_messages::HostBatteryRequestHostStateRequest::None,
            modules: [BatteryModule::default(); 10],
            charge_request: None,
            pack_diagnostic_connect: PackDiagnosticConnect::default(),
            pack_hv_status: PackHvStatus::default(),
            pack_soc: PackSOC::default(),
        }
    }
}

#[derive(Debug)]
pub struct App {
    can_socket_rx: tokio_socketcan::CANSocket,
    can_socket_tx: tokio_socketcan::CANSocket,
    battery_pack: BatteryPack,
}

impl App {
    pub fn new(can_interface: &str) -> Result<Self, eyre::Report> {
        Ok(Self {
            can_socket_rx: tokio_socketcan::CANSocket::open(can_interface)?,
            can_socket_tx: tokio_socketcan::CANSocket::open(can_interface)?,
            battery_pack: BatteryPack::default(),
        })
    }

    pub async fn sleep(&mut self) -> Result<(), eyre::Report> {
        self.send_mode_command_raw(
            abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Sleep,
        )
        .await
    }

    pub async fn run(&mut self, mut terminal: tui::Tui) -> Result<(), eyre::Report> {
        // Initial setup so it's snappy on startup.
        terminal.draw(|frame| self.render_frame(frame))?;
        let _ = self.send_mode_command().await?;

        let timeout = tokio::time::sleep(tokio::time::Duration::from_secs(1)).fuse();
        tokio::pin!(timeout);

        let mut event_reader = crossterm::event::EventStream::new();

        let need_redraw = tokio::sync::Notify::new();

        loop {
            tokio::select! {
                biased;

                maybe_frame = self.can_socket_rx.next() => {
                    match maybe_frame {
                        Some(Ok(frame)) => {
                            let _ = self.handle_can_frame(frame);
                            need_redraw.notify_one();
                            if timeout.is_terminated() {
                                timeout.set(tokio::time::sleep(tokio::time::Duration::from_secs(1)).fuse());
                            }
                        }
                        _ => ()
                    }
                }

                maybe_event = event_reader.next() => {
                    match maybe_event {
                        Some(Ok(crossterm::event::Event::Key(key))) => {
                            match key.code {
                                crossterm::event::KeyCode::Char('q') => {
                                    break Ok(());
                                }
                                crossterm::event::KeyCode::Char('s') => {
                                    self.battery_pack.mode = abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Sleep;
                                    // The Sleep mode is special.  We need
                                    // to send the Sleep command once and
                                    // then not again, or the subsequent
                                    // Sleep commands will wake the pack
                                    // up briefly to respond to the Sleep.
                                    let _ = self.sleep().await?;
                                }
                                crossterm::event::KeyCode::Char('c') => {
                                    self.battery_pack.mode = abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Charge;
                                }
                                crossterm::event::KeyCode::Char('d') => {
                                    self.battery_pack.mode = abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Drive;
                                }
                                crossterm::event::KeyCode::Char('n') => {
                                    self.battery_pack.mode = abs_alliance_can_messages::HostBatteryRequestHostStateRequest::None;
                                }
                                _ => (),
                            }
                            // Send the new mode command right away
                            // (don't wait for the next timeout).
                            // This makes it more responsive to user
                            // input.  `send_mode_command()` does *not*
                            // send a packet if we're in Sleep mode.
                            let _ = self.send_mode_command().await?;

                            // If we had put the pack to Sleep and turned
                            // timeouts ticks off, restart the timout
                            // future now.
                            if timeout.is_terminated() {
                                timeout.set(tokio::time::sleep(tokio::time::Duration::from_secs(1)).fuse());
                            }
                            need_redraw.notify_one();
                        }
                        Some(Ok(_)) => (),
                        Some(Err(e)) => break Err(eyre::eyre!("event error: {:?}", e)),
                        None => break Err(eyre::eyre!("no event!")),
                    }
                }

                _ = need_redraw.notified() => {
                    terminal.draw(|frame| self.render_frame(frame))?;
                }

                _ = &mut timeout => {
                    let _ = self.send_mode_command().await?;

                    let now = std::time::Instant::now();

                    // This keeps us awake if we still have things on
                    // the screen that might need to be timed out.
                    let mut need_to_stay_awake = false;

                    // Time out battery modules.
                    for battery_module in &mut self.battery_pack.modules {
                        if let Some(last_seen) = battery_module.last_seen {
                            if (now - last_seen) < std::time::Duration::from_secs(2) {
                                // This module is still there, check again next tick.
                                need_to_stay_awake = true;
                            } else {
                                battery_module.last_seen = None;
                                need_redraw.notify_one();
                            }
                        }
                    }

                    // Time out the pack charge request.
                    if let Some(charge_request) = self.battery_pack.charge_request {
                        if (now - charge_request.last_seen) < std::time::Duration::from_secs(2) {
                            // The Charge Request is still there, check again next tick.
                            need_to_stay_awake = true;
                        } else {
                            self.battery_pack.charge_request = None;
                            need_redraw.notify_one();
                        }
                    }

                    // Set the next tick timeout, if needed.  We disable
                    // our internal timer when we're in Sleep mode
                    // and everything has timed out, so we don't have
                    // anything to do.
                    match need_to_stay_awake {
                        true => timeout.set(tokio::time::sleep(tokio::time::Duration::from_secs(1)).fuse()),
                        false => timeout.set(futures::future::Fuse::terminated()),
                    }
                }
            }
        }
    }
}

impl App {
    fn render_frame(&self, frame: &mut ratatui::Frame) {
        frame.render_widget(self, frame.area());
    }

    fn handle_can_frame(&mut self, frame: tokio_socketcan::CANFrame) -> Result<(), eyre::Report> {
        let id: embedded_can::Id = if frame.is_extended() {
            match embedded_can::ExtendedId::new(frame.id()) {
                Some(id) => embedded_can::Id::Extended(id),
                None => return Err(eyre::eyre!("invalid extended frame id {}", frame.id())),
            }
        } else {
            match embedded_can::StandardId::new(frame.id() as u16) {
                Some(id) => embedded_can::Id::Standard(id),
                None => return Err(eyre::eyre!("invalid standard frame id {}", frame.id())),
            }
        };

        let msg = abs_alliance_can_messages::Messages::from_can_message(id, frame.data())?;
        // println!("{:#?}", msg);

        match msg {
            abs_alliance_can_messages::Messages::BattDeviceInfo0(m) => {
                self.battery_pack.modules[0].serial_number = m.batt_serial_number_0();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo1(m) => {
                self.battery_pack.modules[1].serial_number = m.batt_serial_number_1();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo2(m) => {
                self.battery_pack.modules[2].serial_number = m.batt_serial_number_2();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo3(m) => {
                self.battery_pack.modules[3].serial_number = m.batt_serial_number_3();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo4(m) => {
                self.battery_pack.modules[4].serial_number = m.batt_serial_number_4();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo5(m) => {
                self.battery_pack.modules[5].serial_number = m.batt_serial_number_5();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo6(m) => {
                self.battery_pack.modules[6].serial_number = m.batt_serial_number_6();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDeviceInfo7(m) => {
                self.battery_pack.modules[7].serial_number = m.batt_serial_number_7();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattBoardAdc10(m) => {
                self.battery_pack.modules[0].adc1.pack_voltage = m.batt_board_adc_load_0();
                self.battery_pack.modules[0].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_0();
                self.battery_pack.modules[0].adc1.load_voltage = m.batt_board_adc_load_0();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc11(m) => {
                self.battery_pack.modules[1].adc1.pack_voltage = m.batt_board_adc_load_1();
                self.battery_pack.modules[1].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_1();
                self.battery_pack.modules[1].adc1.load_voltage = m.batt_board_adc_load_1();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc12(m) => {
                self.battery_pack.modules[2].adc1.pack_voltage = m.batt_board_adc_load_2();
                self.battery_pack.modules[2].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_2();
                self.battery_pack.modules[2].adc1.load_voltage = m.batt_board_adc_load_2();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc13(m) => {
                self.battery_pack.modules[3].adc1.pack_voltage = m.batt_board_adc_load_3();
                self.battery_pack.modules[3].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_3();
                self.battery_pack.modules[3].adc1.load_voltage = m.batt_board_adc_load_3();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc14(m) => {
                self.battery_pack.modules[4].adc1.pack_voltage = m.batt_board_adc_load_4();
                self.battery_pack.modules[4].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_4();
                self.battery_pack.modules[4].adc1.load_voltage = m.batt_board_adc_load_4();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc15(m) => {
                self.battery_pack.modules[5].adc1.pack_voltage = m.batt_board_adc_load_5();
                self.battery_pack.modules[5].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_5();
                self.battery_pack.modules[5].adc1.load_voltage = m.batt_board_adc_load_5();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc16(m) => {
                self.battery_pack.modules[6].adc1.pack_voltage = m.batt_board_adc_load_6();
                self.battery_pack.modules[6].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_6();
                self.battery_pack.modules[6].adc1.load_voltage = m.batt_board_adc_load_6();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc17(m) => {
                self.battery_pack.modules[7].adc1.pack_voltage = m.batt_board_adc_load_7();
                self.battery_pack.modules[7].adc1.common_drain_voltage =
                    m.batt_board_adc_common_drain_7();
                self.battery_pack.modules[7].adc1.load_voltage = m.batt_board_adc_load_7();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattBoardAdc20(m) => {
                self.battery_pack.modules[0].adc2.rail_5v = m.batt_board_adc_5v_0();
                self.battery_pack.modules[0].adc2.rail_12v = m.batt_board_adc_12v_0();
                self.battery_pack.modules[0].adc2.rail_3v3 = m.batt_board_adc_3v3_0();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc21(m) => {
                self.battery_pack.modules[1].adc2.rail_5v = m.batt_board_adc_5v_1();
                self.battery_pack.modules[1].adc2.rail_12v = m.batt_board_adc_12v_1();
                self.battery_pack.modules[1].adc2.rail_3v3 = m.batt_board_adc_3v3_1();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc22(m) => {
                self.battery_pack.modules[2].adc2.rail_5v = m.batt_board_adc_5v_2();
                self.battery_pack.modules[2].adc2.rail_12v = m.batt_board_adc_12v_2();
                self.battery_pack.modules[2].adc2.rail_3v3 = m.batt_board_adc_3v3_2();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc23(m) => {
                self.battery_pack.modules[3].adc2.rail_5v = m.batt_board_adc_5v_3();
                self.battery_pack.modules[3].adc2.rail_12v = m.batt_board_adc_12v_3();
                self.battery_pack.modules[3].adc2.rail_3v3 = m.batt_board_adc_3v3_3();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc24(m) => {
                self.battery_pack.modules[4].adc2.rail_5v = m.batt_board_adc_5v_4();
                self.battery_pack.modules[4].adc2.rail_12v = m.batt_board_adc_12v_4();
                self.battery_pack.modules[4].adc2.rail_3v3 = m.batt_board_adc_3v3_4();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc25(m) => {
                self.battery_pack.modules[5].adc2.rail_5v = m.batt_board_adc_5v_5();
                self.battery_pack.modules[5].adc2.rail_12v = m.batt_board_adc_12v_5();
                self.battery_pack.modules[5].adc2.rail_3v3 = m.batt_board_adc_3v3_5();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc26(m) => {
                self.battery_pack.modules[6].adc2.rail_5v = m.batt_board_adc_5v_6();
                self.battery_pack.modules[6].adc2.rail_12v = m.batt_board_adc_12v_6();
                self.battery_pack.modules[6].adc2.rail_3v3 = m.batt_board_adc_3v3_6();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattBoardAdc27(m) => {
                self.battery_pack.modules[7].adc2.rail_5v = m.batt_board_adc_5v_7();
                self.battery_pack.modules[7].adc2.rail_12v = m.batt_board_adc_12v_7();
                self.battery_pack.modules[7].adc2.rail_3v3 = m.batt_board_adc_3v3_7();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattModHvStatus0(m) => {
                self.battery_pack.modules[0].hv_status.voltage = m.batt_v_mod_0_raw();
                self.battery_pack.modules[0].hv_status.current = m.batt_i_mod_filtered_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus1(m) => {
                self.battery_pack.modules[1].hv_status.voltage = m.batt_v_mod_1_raw();
                self.battery_pack.modules[1].hv_status.current = m.batt_i_mod_filtered_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus2(m) => {
                self.battery_pack.modules[2].hv_status.voltage = m.batt_v_mod_2_raw();
                self.battery_pack.modules[2].hv_status.current = m.batt_i_mod_filtered_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus3(m) => {
                self.battery_pack.modules[3].hv_status.voltage = m.batt_v_mod_3_raw();
                self.battery_pack.modules[3].hv_status.current = m.batt_i_mod_filtered_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus4(m) => {
                self.battery_pack.modules[4].hv_status.voltage = m.batt_v_mod_4_raw();
                self.battery_pack.modules[4].hv_status.current = m.batt_i_mod_filtered_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus5(m) => {
                self.battery_pack.modules[5].hv_status.voltage = m.batt_v_mod_5_raw();
                self.battery_pack.modules[5].hv_status.current = m.batt_i_mod_filtered_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus6(m) => {
                self.battery_pack.modules[6].hv_status.voltage = m.batt_v_mod_6_raw();
                self.battery_pack.modules[6].hv_status.current = m.batt_i_mod_filtered_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModHvStatus7(m) => {
                self.battery_pack.modules[7].hv_status.voltage = m.batt_v_mod_7_raw();
                self.battery_pack.modules[7].hv_status.current = m.batt_i_mod_filtered_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattModChgLimits0(m) => {
                self.battery_pack.modules[0].charge_limit.voltage = m.batt_v_mod_chg_limit_0_raw();
                self.battery_pack.modules[0].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits1(m) => {
                self.battery_pack.modules[1].charge_limit.voltage = m.batt_v_mod_chg_limit_1_raw();
                self.battery_pack.modules[1].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits2(m) => {
                self.battery_pack.modules[2].charge_limit.voltage = m.batt_v_mod_chg_limit_2_raw();
                self.battery_pack.modules[2].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits3(m) => {
                self.battery_pack.modules[3].charge_limit.voltage = m.batt_v_mod_chg_limit_3_raw();
                self.battery_pack.modules[3].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits4(m) => {
                self.battery_pack.modules[4].charge_limit.voltage = m.batt_v_mod_chg_limit_4_raw();
                self.battery_pack.modules[4].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits5(m) => {
                self.battery_pack.modules[5].charge_limit.voltage = m.batt_v_mod_chg_limit_5_raw();
                self.battery_pack.modules[5].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits6(m) => {
                self.battery_pack.modules[6].charge_limit.voltage = m.batt_v_mod_chg_limit_6_raw();
                self.battery_pack.modules[6].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModChgLimits7(m) => {
                self.battery_pack.modules[7].charge_limit.voltage = m.batt_v_mod_chg_limit_7_raw();
                self.battery_pack.modules[7].charge_limit.current =
                    m.batt_i_mod_chg_limit_cont_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattModSoc0(m) => {
                self.battery_pack.modules[0].soc.soc = m.batt_soc_mod_0_raw();
                self.battery_pack.modules[0].soc.soh = m.batt_sohc_mod_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc1(m) => {
                self.battery_pack.modules[1].soc.soc = m.batt_soc_mod_1_raw();
                self.battery_pack.modules[1].soc.soh = m.batt_sohc_mod_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc2(m) => {
                self.battery_pack.modules[2].soc.soc = m.batt_soc_mod_2_raw();
                self.battery_pack.modules[2].soc.soh = m.batt_sohc_mod_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc3(m) => {
                self.battery_pack.modules[3].soc.soc = m.batt_soc_mod_3_raw();
                self.battery_pack.modules[3].soc.soh = m.batt_sohc_mod_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc4(m) => {
                self.battery_pack.modules[4].soc.soc = m.batt_soc_mod_4_raw();
                self.battery_pack.modules[4].soc.soh = m.batt_sohc_mod_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc5(m) => {
                self.battery_pack.modules[5].soc.soc = m.batt_soc_mod_5_raw();
                self.battery_pack.modules[5].soc.soh = m.batt_sohc_mod_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc6(m) => {
                self.battery_pack.modules[6].soc.soc = m.batt_soc_mod_6_raw();
                self.battery_pack.modules[6].soc.soh = m.batt_sohc_mod_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModSoc7(m) => {
                self.battery_pack.modules[7].soc.soc = m.batt_soc_mod_7_raw();
                self.battery_pack.modules[7].soc.soh = m.batt_sohc_mod_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattModTemperaturesA0(m) => {
                self.battery_pack.modules[0].temperatures_a.ambient = m.batt_t_ambient_0_raw();
                self.battery_pack.modules[0].temperatures_a.module1 = m.batt_t_module1_0_raw();
                self.battery_pack.modules[0].temperatures_a.module2 = m.batt_t_module2_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA1(m) => {
                self.battery_pack.modules[1].temperatures_a.ambient = m.batt_t_ambient_1_raw();
                self.battery_pack.modules[1].temperatures_a.module1 = m.batt_t_module1_1_raw();
                self.battery_pack.modules[1].temperatures_a.module2 = m.batt_t_module2_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA2(m) => {
                self.battery_pack.modules[2].temperatures_a.ambient = m.batt_t_ambient_2_raw();
                self.battery_pack.modules[2].temperatures_a.module1 = m.batt_t_module1_2_raw();
                self.battery_pack.modules[2].temperatures_a.module2 = m.batt_t_module2_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA3(m) => {
                self.battery_pack.modules[3].temperatures_a.ambient = m.batt_t_ambient_3_raw();
                self.battery_pack.modules[3].temperatures_a.module1 = m.batt_t_module1_3_raw();
                self.battery_pack.modules[3].temperatures_a.module2 = m.batt_t_module2_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA4(m) => {
                self.battery_pack.modules[4].temperatures_a.ambient = m.batt_t_ambient_4_raw();
                self.battery_pack.modules[4].temperatures_a.module1 = m.batt_t_module1_4_raw();
                self.battery_pack.modules[4].temperatures_a.module2 = m.batt_t_module2_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA5(m) => {
                self.battery_pack.modules[5].temperatures_a.ambient = m.batt_t_ambient_5_raw();
                self.battery_pack.modules[5].temperatures_a.module1 = m.batt_t_module1_5_raw();
                self.battery_pack.modules[5].temperatures_a.module2 = m.batt_t_module2_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA6(m) => {
                self.battery_pack.modules[6].temperatures_a.ambient = m.batt_t_ambient_6_raw();
                self.battery_pack.modules[6].temperatures_a.module1 = m.batt_t_module1_6_raw();
                self.battery_pack.modules[6].temperatures_a.module2 = m.batt_t_module2_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesA7(m) => {
                self.battery_pack.modules[7].temperatures_a.ambient = m.batt_t_ambient_7_raw();
                self.battery_pack.modules[7].temperatures_a.module1 = m.batt_t_module1_7_raw();
                self.battery_pack.modules[7].temperatures_a.module2 = m.batt_t_module2_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattModTemperaturesB0(m) => {
                self.battery_pack.modules[0].temperatures_b.fet = m.batt_t_fet_0_raw();
                self.battery_pack.modules[0].temperatures_b.shunt = m.batt_t_shunt_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB1(m) => {
                self.battery_pack.modules[1].temperatures_b.fet = m.batt_t_fet_1_raw();
                self.battery_pack.modules[1].temperatures_b.shunt = m.batt_t_shunt_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB2(m) => {
                self.battery_pack.modules[2].temperatures_b.fet = m.batt_t_fet_2_raw();
                self.battery_pack.modules[2].temperatures_b.shunt = m.batt_t_shunt_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB3(m) => {
                self.battery_pack.modules[3].temperatures_b.fet = m.batt_t_fet_3_raw();
                self.battery_pack.modules[3].temperatures_b.shunt = m.batt_t_shunt_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB4(m) => {
                self.battery_pack.modules[4].temperatures_b.fet = m.batt_t_fet_4_raw();
                self.battery_pack.modules[4].temperatures_b.shunt = m.batt_t_shunt_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB5(m) => {
                self.battery_pack.modules[5].temperatures_b.fet = m.batt_t_fet_5_raw();
                self.battery_pack.modules[5].temperatures_b.shunt = m.batt_t_shunt_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB6(m) => {
                self.battery_pack.modules[6].temperatures_b.fet = m.batt_t_fet_6_raw();
                self.battery_pack.modules[6].temperatures_b.shunt = m.batt_t_shunt_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattModTemperaturesB7(m) => {
                self.battery_pack.modules[7].temperatures_b.fet = m.batt_t_fet_7_raw();
                self.battery_pack.modules[7].temperatures_b.shunt = m.batt_t_shunt_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA0(m) => {
                self.battery_pack.modules[0].v_bricks[0] = m.batt_v_brick01_0_raw();
                self.battery_pack.modules[0].v_bricks[1] = m.batt_v_brick02_0_raw();
                self.battery_pack.modules[0].v_bricks[2] = m.batt_v_brick03_0_raw();
                self.battery_pack.modules[0].v_bricks[3] = m.batt_v_brick04_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA1(m) => {
                self.battery_pack.modules[1].v_bricks[0] = m.batt_v_brick01_1_raw();
                self.battery_pack.modules[1].v_bricks[1] = m.batt_v_brick02_1_raw();
                self.battery_pack.modules[1].v_bricks[2] = m.batt_v_brick03_1_raw();
                self.battery_pack.modules[1].v_bricks[3] = m.batt_v_brick04_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA2(m) => {
                self.battery_pack.modules[2].v_bricks[0] = m.batt_v_brick01_2_raw();
                self.battery_pack.modules[2].v_bricks[1] = m.batt_v_brick02_2_raw();
                self.battery_pack.modules[2].v_bricks[2] = m.batt_v_brick03_2_raw();
                self.battery_pack.modules[2].v_bricks[3] = m.batt_v_brick04_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA3(m) => {
                self.battery_pack.modules[3].v_bricks[0] = m.batt_v_brick01_3_raw();
                self.battery_pack.modules[3].v_bricks[1] = m.batt_v_brick02_3_raw();
                self.battery_pack.modules[3].v_bricks[2] = m.batt_v_brick03_3_raw();
                self.battery_pack.modules[3].v_bricks[3] = m.batt_v_brick04_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA4(m) => {
                self.battery_pack.modules[4].v_bricks[0] = m.batt_v_brick01_4_raw();
                self.battery_pack.modules[4].v_bricks[1] = m.batt_v_brick02_4_raw();
                self.battery_pack.modules[4].v_bricks[2] = m.batt_v_brick03_4_raw();
                self.battery_pack.modules[4].v_bricks[3] = m.batt_v_brick04_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA5(m) => {
                self.battery_pack.modules[5].v_bricks[0] = m.batt_v_brick01_5_raw();
                self.battery_pack.modules[5].v_bricks[1] = m.batt_v_brick02_5_raw();
                self.battery_pack.modules[5].v_bricks[2] = m.batt_v_brick03_5_raw();
                self.battery_pack.modules[5].v_bricks[3] = m.batt_v_brick04_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA6(m) => {
                self.battery_pack.modules[6].v_bricks[0] = m.batt_v_brick01_6_raw();
                self.battery_pack.modules[6].v_bricks[1] = m.batt_v_brick02_6_raw();
                self.battery_pack.modules[6].v_bricks[2] = m.batt_v_brick03_6_raw();
                self.battery_pack.modules[6].v_bricks[3] = m.batt_v_brick04_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksA7(m) => {
                self.battery_pack.modules[7].v_bricks[0] = m.batt_v_brick01_7_raw();
                self.battery_pack.modules[7].v_bricks[1] = m.batt_v_brick02_7_raw();
                self.battery_pack.modules[7].v_bricks[2] = m.batt_v_brick03_7_raw();
                self.battery_pack.modules[7].v_bricks[3] = m.batt_v_brick04_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB0(m) => {
                self.battery_pack.modules[0].v_bricks[4] = m.batt_v_brick05_0_raw();
                self.battery_pack.modules[0].v_bricks[5] = m.batt_v_brick06_0_raw();
                self.battery_pack.modules[0].v_bricks[6] = m.batt_v_brick07_0_raw();
                self.battery_pack.modules[0].v_bricks[7] = m.batt_v_brick08_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB1(m) => {
                self.battery_pack.modules[1].v_bricks[4] = m.batt_v_brick05_1_raw();
                self.battery_pack.modules[1].v_bricks[5] = m.batt_v_brick06_1_raw();
                self.battery_pack.modules[1].v_bricks[6] = m.batt_v_brick07_1_raw();
                self.battery_pack.modules[1].v_bricks[7] = m.batt_v_brick08_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB2(m) => {
                self.battery_pack.modules[2].v_bricks[4] = m.batt_v_brick05_2_raw();
                self.battery_pack.modules[2].v_bricks[5] = m.batt_v_brick06_2_raw();
                self.battery_pack.modules[2].v_bricks[6] = m.batt_v_brick07_2_raw();
                self.battery_pack.modules[2].v_bricks[7] = m.batt_v_brick08_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB3(m) => {
                self.battery_pack.modules[3].v_bricks[4] = m.batt_v_brick05_3_raw();
                self.battery_pack.modules[3].v_bricks[5] = m.batt_v_brick06_3_raw();
                self.battery_pack.modules[3].v_bricks[6] = m.batt_v_brick07_3_raw();
                self.battery_pack.modules[3].v_bricks[7] = m.batt_v_brick08_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB4(m) => {
                self.battery_pack.modules[4].v_bricks[4] = m.batt_v_brick05_4_raw();
                self.battery_pack.modules[4].v_bricks[5] = m.batt_v_brick06_4_raw();
                self.battery_pack.modules[4].v_bricks[6] = m.batt_v_brick07_4_raw();
                self.battery_pack.modules[4].v_bricks[7] = m.batt_v_brick08_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB5(m) => {
                self.battery_pack.modules[5].v_bricks[4] = m.batt_v_brick05_5_raw();
                self.battery_pack.modules[5].v_bricks[5] = m.batt_v_brick06_5_raw();
                self.battery_pack.modules[5].v_bricks[6] = m.batt_v_brick07_5_raw();
                self.battery_pack.modules[5].v_bricks[7] = m.batt_v_brick08_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB6(m) => {
                self.battery_pack.modules[6].v_bricks[4] = m.batt_v_brick05_6_raw();
                self.battery_pack.modules[6].v_bricks[5] = m.batt_v_brick06_6_raw();
                self.battery_pack.modules[6].v_bricks[6] = m.batt_v_brick07_6_raw();
                self.battery_pack.modules[6].v_bricks[7] = m.batt_v_brick08_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksB7(m) => {
                self.battery_pack.modules[7].v_bricks[4] = m.batt_v_brick05_7_raw();
                self.battery_pack.modules[7].v_bricks[5] = m.batt_v_brick06_7_raw();
                self.battery_pack.modules[7].v_bricks[6] = m.batt_v_brick07_7_raw();
                self.battery_pack.modules[7].v_bricks[7] = m.batt_v_brick08_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC0(m) => {
                self.battery_pack.modules[0].v_bricks[8] = m.batt_v_brick09_0_raw();
                self.battery_pack.modules[0].v_bricks[9] = m.batt_v_brick10_0_raw();
                self.battery_pack.modules[0].v_bricks[10] = m.batt_v_brick11_0_raw();
                self.battery_pack.modules[0].v_bricks[11] = m.batt_v_brick12_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC1(m) => {
                self.battery_pack.modules[1].v_bricks[8] = m.batt_v_brick09_1_raw();
                self.battery_pack.modules[1].v_bricks[9] = m.batt_v_brick10_1_raw();
                self.battery_pack.modules[1].v_bricks[10] = m.batt_v_brick11_1_raw();
                self.battery_pack.modules[1].v_bricks[11] = m.batt_v_brick12_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC2(m) => {
                self.battery_pack.modules[2].v_bricks[8] = m.batt_v_brick09_2_raw();
                self.battery_pack.modules[2].v_bricks[9] = m.batt_v_brick10_2_raw();
                self.battery_pack.modules[2].v_bricks[10] = m.batt_v_brick11_2_raw();
                self.battery_pack.modules[2].v_bricks[11] = m.batt_v_brick12_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC3(m) => {
                self.battery_pack.modules[3].v_bricks[8] = m.batt_v_brick09_3_raw();
                self.battery_pack.modules[3].v_bricks[9] = m.batt_v_brick10_3_raw();
                self.battery_pack.modules[3].v_bricks[10] = m.batt_v_brick11_3_raw();
                self.battery_pack.modules[3].v_bricks[11] = m.batt_v_brick12_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC4(m) => {
                self.battery_pack.modules[4].v_bricks[8] = m.batt_v_brick09_4_raw();
                self.battery_pack.modules[4].v_bricks[9] = m.batt_v_brick10_4_raw();
                self.battery_pack.modules[4].v_bricks[10] = m.batt_v_brick11_4_raw();
                self.battery_pack.modules[4].v_bricks[11] = m.batt_v_brick12_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC5(m) => {
                self.battery_pack.modules[5].v_bricks[8] = m.batt_v_brick09_5_raw();
                self.battery_pack.modules[5].v_bricks[9] = m.batt_v_brick10_5_raw();
                self.battery_pack.modules[5].v_bricks[10] = m.batt_v_brick11_5_raw();
                self.battery_pack.modules[5].v_bricks[11] = m.batt_v_brick12_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC6(m) => {
                self.battery_pack.modules[6].v_bricks[8] = m.batt_v_brick09_6_raw();
                self.battery_pack.modules[6].v_bricks[9] = m.batt_v_brick10_6_raw();
                self.battery_pack.modules[6].v_bricks[10] = m.batt_v_brick11_6_raw();
                self.battery_pack.modules[6].v_bricks[11] = m.batt_v_brick12_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksC7(m) => {
                self.battery_pack.modules[7].v_bricks[8] = m.batt_v_brick09_7_raw();
                self.battery_pack.modules[7].v_bricks[9] = m.batt_v_brick10_7_raw();
                self.battery_pack.modules[7].v_bricks[10] = m.batt_v_brick11_7_raw();
                self.battery_pack.modules[7].v_bricks[11] = m.batt_v_brick12_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD0(m) => {
                self.battery_pack.modules[0].v_bricks[12] = m.batt_v_brick13_0_raw();
                self.battery_pack.modules[0].v_bricks[13] = m.batt_v_brick14_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD1(m) => {
                self.battery_pack.modules[1].v_bricks[12] = m.batt_v_brick13_1_raw();
                self.battery_pack.modules[1].v_bricks[13] = m.batt_v_brick14_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD2(m) => {
                self.battery_pack.modules[2].v_bricks[12] = m.batt_v_brick13_2_raw();
                self.battery_pack.modules[2].v_bricks[13] = m.batt_v_brick14_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD3(m) => {
                self.battery_pack.modules[3].v_bricks[12] = m.batt_v_brick13_3_raw();
                self.battery_pack.modules[3].v_bricks[13] = m.batt_v_brick14_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD4(m) => {
                self.battery_pack.modules[4].v_bricks[12] = m.batt_v_brick13_4_raw();
                self.battery_pack.modules[4].v_bricks[13] = m.batt_v_brick14_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD5(m) => {
                self.battery_pack.modules[5].v_bricks[12] = m.batt_v_brick13_5_raw();
                self.battery_pack.modules[5].v_bricks[13] = m.batt_v_brick14_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD6(m) => {
                self.battery_pack.modules[6].v_bricks[12] = m.batt_v_brick13_6_raw();
                self.battery_pack.modules[6].v_bricks[13] = m.batt_v_brick14_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticVBricksD7(m) => {
                self.battery_pack.modules[7].v_bricks[12] = m.batt_v_brick13_7_raw();
                self.battery_pack.modules[7].v_bricks[13] = m.batt_v_brick14_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick0(m) => {
                self.battery_pack.modules[0].balancing[0] = m.batt_bal_status_brick01_0_raw();
                self.battery_pack.modules[0].balancing[1] = m.batt_bal_status_brick02_0_raw();
                self.battery_pack.modules[0].balancing[2] = m.batt_bal_status_brick03_0_raw();
                self.battery_pack.modules[0].balancing[3] = m.batt_bal_status_brick04_0_raw();
                self.battery_pack.modules[0].balancing[4] = m.batt_bal_status_brick05_0_raw();
                self.battery_pack.modules[0].balancing[5] = m.batt_bal_status_brick06_0_raw();
                self.battery_pack.modules[0].balancing[6] = m.batt_bal_status_brick07_0_raw();
                self.battery_pack.modules[0].balancing[7] = m.batt_bal_status_brick08_0_raw();
                self.battery_pack.modules[0].balancing[8] = m.batt_bal_status_brick09_0_raw();
                self.battery_pack.modules[0].balancing[9] = m.batt_bal_status_brick10_0_raw();
                self.battery_pack.modules[0].balancing[10] = m.batt_bal_status_brick11_0_raw();
                self.battery_pack.modules[0].balancing[11] = m.batt_bal_status_brick12_0_raw();
                self.battery_pack.modules[0].balancing[12] = m.batt_bal_status_brick13_0_raw();
                self.battery_pack.modules[0].balancing[13] = m.batt_bal_status_brick14_0_raw();
                self.battery_pack.modules[0].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick1(m) => {
                self.battery_pack.modules[1].balancing[0] = m.batt_bal_status_brick01_1_raw();
                self.battery_pack.modules[1].balancing[1] = m.batt_bal_status_brick02_1_raw();
                self.battery_pack.modules[1].balancing[2] = m.batt_bal_status_brick03_1_raw();
                self.battery_pack.modules[1].balancing[3] = m.batt_bal_status_brick04_1_raw();
                self.battery_pack.modules[1].balancing[4] = m.batt_bal_status_brick05_1_raw();
                self.battery_pack.modules[1].balancing[5] = m.batt_bal_status_brick06_1_raw();
                self.battery_pack.modules[1].balancing[6] = m.batt_bal_status_brick07_1_raw();
                self.battery_pack.modules[1].balancing[7] = m.batt_bal_status_brick08_1_raw();
                self.battery_pack.modules[1].balancing[8] = m.batt_bal_status_brick09_1_raw();
                self.battery_pack.modules[1].balancing[9] = m.batt_bal_status_brick10_1_raw();
                self.battery_pack.modules[1].balancing[10] = m.batt_bal_status_brick11_1_raw();
                self.battery_pack.modules[1].balancing[11] = m.batt_bal_status_brick12_1_raw();
                self.battery_pack.modules[1].balancing[12] = m.batt_bal_status_brick13_1_raw();
                self.battery_pack.modules[1].balancing[13] = m.batt_bal_status_brick14_1_raw();
                self.battery_pack.modules[1].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick2(m) => {
                self.battery_pack.modules[2].balancing[0] = m.batt_bal_status_brick01_2_raw();
                self.battery_pack.modules[2].balancing[1] = m.batt_bal_status_brick02_2_raw();
                self.battery_pack.modules[2].balancing[2] = m.batt_bal_status_brick03_2_raw();
                self.battery_pack.modules[2].balancing[3] = m.batt_bal_status_brick04_2_raw();
                self.battery_pack.modules[2].balancing[4] = m.batt_bal_status_brick05_2_raw();
                self.battery_pack.modules[2].balancing[5] = m.batt_bal_status_brick06_2_raw();
                self.battery_pack.modules[2].balancing[6] = m.batt_bal_status_brick07_2_raw();
                self.battery_pack.modules[2].balancing[7] = m.batt_bal_status_brick08_2_raw();
                self.battery_pack.modules[2].balancing[8] = m.batt_bal_status_brick09_2_raw();
                self.battery_pack.modules[2].balancing[9] = m.batt_bal_status_brick10_2_raw();
                self.battery_pack.modules[2].balancing[10] = m.batt_bal_status_brick11_2_raw();
                self.battery_pack.modules[2].balancing[11] = m.batt_bal_status_brick12_2_raw();
                self.battery_pack.modules[2].balancing[12] = m.batt_bal_status_brick13_2_raw();
                self.battery_pack.modules[2].balancing[13] = m.batt_bal_status_brick14_2_raw();
                self.battery_pack.modules[2].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick3(m) => {
                self.battery_pack.modules[3].balancing[0] = m.batt_bal_status_brick01_3_raw();
                self.battery_pack.modules[3].balancing[1] = m.batt_bal_status_brick02_3_raw();
                self.battery_pack.modules[3].balancing[2] = m.batt_bal_status_brick03_3_raw();
                self.battery_pack.modules[3].balancing[3] = m.batt_bal_status_brick04_3_raw();
                self.battery_pack.modules[3].balancing[4] = m.batt_bal_status_brick05_3_raw();
                self.battery_pack.modules[3].balancing[5] = m.batt_bal_status_brick06_3_raw();
                self.battery_pack.modules[3].balancing[6] = m.batt_bal_status_brick07_3_raw();
                self.battery_pack.modules[3].balancing[7] = m.batt_bal_status_brick08_3_raw();
                self.battery_pack.modules[3].balancing[8] = m.batt_bal_status_brick09_3_raw();
                self.battery_pack.modules[3].balancing[9] = m.batt_bal_status_brick10_3_raw();
                self.battery_pack.modules[3].balancing[10] = m.batt_bal_status_brick11_3_raw();
                self.battery_pack.modules[3].balancing[11] = m.batt_bal_status_brick12_3_raw();
                self.battery_pack.modules[3].balancing[12] = m.batt_bal_status_brick13_3_raw();
                self.battery_pack.modules[3].balancing[13] = m.batt_bal_status_brick14_3_raw();
                self.battery_pack.modules[3].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick4(m) => {
                self.battery_pack.modules[4].balancing[0] = m.batt_bal_status_brick01_4_raw();
                self.battery_pack.modules[4].balancing[1] = m.batt_bal_status_brick02_4_raw();
                self.battery_pack.modules[4].balancing[2] = m.batt_bal_status_brick03_4_raw();
                self.battery_pack.modules[4].balancing[3] = m.batt_bal_status_brick04_4_raw();
                self.battery_pack.modules[4].balancing[4] = m.batt_bal_status_brick05_4_raw();
                self.battery_pack.modules[4].balancing[5] = m.batt_bal_status_brick06_4_raw();
                self.battery_pack.modules[4].balancing[6] = m.batt_bal_status_brick07_4_raw();
                self.battery_pack.modules[4].balancing[7] = m.batt_bal_status_brick08_4_raw();
                self.battery_pack.modules[4].balancing[8] = m.batt_bal_status_brick09_4_raw();
                self.battery_pack.modules[4].balancing[9] = m.batt_bal_status_brick10_4_raw();
                self.battery_pack.modules[4].balancing[10] = m.batt_bal_status_brick11_4_raw();
                self.battery_pack.modules[4].balancing[11] = m.batt_bal_status_brick12_4_raw();
                self.battery_pack.modules[4].balancing[12] = m.batt_bal_status_brick13_4_raw();
                self.battery_pack.modules[4].balancing[13] = m.batt_bal_status_brick14_4_raw();
                self.battery_pack.modules[4].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick5(m) => {
                self.battery_pack.modules[5].balancing[0] = m.batt_bal_status_brick01_5_raw();
                self.battery_pack.modules[5].balancing[1] = m.batt_bal_status_brick02_5_raw();
                self.battery_pack.modules[5].balancing[2] = m.batt_bal_status_brick03_5_raw();
                self.battery_pack.modules[5].balancing[3] = m.batt_bal_status_brick04_5_raw();
                self.battery_pack.modules[5].balancing[4] = m.batt_bal_status_brick05_5_raw();
                self.battery_pack.modules[5].balancing[5] = m.batt_bal_status_brick06_5_raw();
                self.battery_pack.modules[5].balancing[6] = m.batt_bal_status_brick07_5_raw();
                self.battery_pack.modules[5].balancing[7] = m.batt_bal_status_brick08_5_raw();
                self.battery_pack.modules[5].balancing[8] = m.batt_bal_status_brick09_5_raw();
                self.battery_pack.modules[5].balancing[9] = m.batt_bal_status_brick10_5_raw();
                self.battery_pack.modules[5].balancing[10] = m.batt_bal_status_brick11_5_raw();
                self.battery_pack.modules[5].balancing[11] = m.batt_bal_status_brick12_5_raw();
                self.battery_pack.modules[5].balancing[12] = m.batt_bal_status_brick13_5_raw();
                self.battery_pack.modules[5].balancing[13] = m.batt_bal_status_brick14_5_raw();
                self.battery_pack.modules[5].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick6(m) => {
                self.battery_pack.modules[6].balancing[0] = m.batt_bal_status_brick01_6_raw();
                self.battery_pack.modules[6].balancing[1] = m.batt_bal_status_brick02_6_raw();
                self.battery_pack.modules[6].balancing[2] = m.batt_bal_status_brick03_6_raw();
                self.battery_pack.modules[6].balancing[3] = m.batt_bal_status_brick04_6_raw();
                self.battery_pack.modules[6].balancing[4] = m.batt_bal_status_brick05_6_raw();
                self.battery_pack.modules[6].balancing[5] = m.batt_bal_status_brick06_6_raw();
                self.battery_pack.modules[6].balancing[6] = m.batt_bal_status_brick07_6_raw();
                self.battery_pack.modules[6].balancing[7] = m.batt_bal_status_brick08_6_raw();
                self.battery_pack.modules[6].balancing[8] = m.batt_bal_status_brick09_6_raw();
                self.battery_pack.modules[6].balancing[9] = m.batt_bal_status_brick10_6_raw();
                self.battery_pack.modules[6].balancing[10] = m.batt_bal_status_brick11_6_raw();
                self.battery_pack.modules[6].balancing[11] = m.batt_bal_status_brick12_6_raw();
                self.battery_pack.modules[6].balancing[12] = m.batt_bal_status_brick13_6_raw();
                self.battery_pack.modules[6].balancing[13] = m.batt_bal_status_brick14_6_raw();
                self.battery_pack.modules[6].last_seen = Some(std::time::Instant::now());
            }
            abs_alliance_can_messages::Messages::BattDiagnosticBalStatusBrick7(m) => {
                self.battery_pack.modules[7].balancing[0] = m.batt_bal_status_brick01_7_raw();
                self.battery_pack.modules[7].balancing[1] = m.batt_bal_status_brick02_7_raw();
                self.battery_pack.modules[7].balancing[2] = m.batt_bal_status_brick03_7_raw();
                self.battery_pack.modules[7].balancing[3] = m.batt_bal_status_brick04_7_raw();
                self.battery_pack.modules[7].balancing[4] = m.batt_bal_status_brick05_7_raw();
                self.battery_pack.modules[7].balancing[5] = m.batt_bal_status_brick06_7_raw();
                self.battery_pack.modules[7].balancing[6] = m.batt_bal_status_brick07_7_raw();
                self.battery_pack.modules[7].balancing[7] = m.batt_bal_status_brick08_7_raw();
                self.battery_pack.modules[7].balancing[8] = m.batt_bal_status_brick09_7_raw();
                self.battery_pack.modules[7].balancing[9] = m.batt_bal_status_brick10_7_raw();
                self.battery_pack.modules[7].balancing[10] = m.batt_bal_status_brick11_7_raw();
                self.battery_pack.modules[7].balancing[11] = m.batt_bal_status_brick12_7_raw();
                self.battery_pack.modules[7].balancing[12] = m.batt_bal_status_brick13_7_raw();
                self.battery_pack.modules[7].balancing[13] = m.batt_bal_status_brick14_7_raw();
                self.battery_pack.modules[7].last_seen = Some(std::time::Instant::now());
            }

            abs_alliance_can_messages::Messages::BattChargerControl(m) => {
                self.battery_pack.charge_request = Some(ChargeRequest {
                    voltage: m.batt_charging_voltage(),
                    current: m.batt_charging_current(),
                    enable: m.batt_charge_enable(),
                    last_seen: std::time::Instant::now(),
                });
            }

            abs_alliance_can_messages::Messages::BattPackDiagnosticConnect(m) => {
                self.battery_pack
                    .pack_diagnostic_connect
                    .num_modules_connected_for_charge = m.batt_pack_mod_chg_entry_conn_ok_m();
                self.battery_pack
                    .pack_diagnostic_connect
                    .num_modules_connected_for_drive = m.batt_pack_mod_drv_entry_conn_ok_m();
                self.battery_pack
                    .pack_diagnostic_connect
                    .num_modules_standby_for_charge = m.batt_pack_mod_chg_standby_conn_ok_m();
                self.battery_pack
                    .pack_diagnostic_connect
                    .num_modules_standby_for_drive = m.batt_pack_mod_drv_standby_conn_ok_m();
            }

            abs_alliance_can_messages::Messages::BattPackHvStatus(m) => {
                self.battery_pack.pack_hv_status.voltage = m.batt_v_pack_raw();
                self.battery_pack.pack_hv_status.current = m.batt_i_pack_filtered_raw();
            }

            abs_alliance_can_messages::Messages::BattPackSoc(m) => {
                self.battery_pack.pack_soc.soc = m.batt_pack_soc_raw();
                self.battery_pack.pack_soc.user_soc = m.batt_pack_user_soc_raw();
            }

            _ => (), // ignore all other messages
        }

        Ok(())
    }

    async fn send_mode_command(&mut self) -> Result<(), eyre::Report> {
        if self.battery_pack.mode
            == abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Sleep
        {
            // Don't send any CAN packets while the battery is in sleep
            // mode, we'd wake it up.
            return Ok(());
        }
        self.send_mode_command_raw(self.battery_pack.mode).await
    }

    async fn send_mode_command_raw(
        &mut self,
        mode: abs_alliance_can_messages::HostBatteryRequestHostStateRequest,
    ) -> Result<(), eyre::Report> {
        let frame = abs_alliance_can_messages::HostBatteryRequest::new(
            false,
            false,
            false,
            false,
            false,
            mode.into(),
        )?;

        let id: u32 = match frame.id() {
            embedded_can::Id::Standard(standard_id) => standard_id.as_raw() as u32,
            embedded_can::Id::Extended(extended_id) => extended_id.as_raw(),
        };
        let raw_frame = tokio_socketcan::CANFrame::new(id, frame.raw(), false, false)?;

        match self.can_socket_tx.write_frame(raw_frame) {
            Ok(can_write_fut) => match can_write_fut.await {
                Ok(_) => return Ok(()),
                Err(_e) => return Ok(()),
            },
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

impl ratatui::widgets::Widget for &BatteryPack {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let mut text = vec![];

        let (sleep_style, charge_style, drive_style, none_style) = match self.mode {
            abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Sleep => (
                ratatui::style::Style::default().bg(ratatui::style::Color::Green),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
            ),
            abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Charge => (
                ratatui::style::Style::default(),
                ratatui::style::Style::default().bg(ratatui::style::Color::Green),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
            ),
            abs_alliance_can_messages::HostBatteryRequestHostStateRequest::Drive => (
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default().bg(ratatui::style::Color::Green),
                ratatui::style::Style::default(),
            ),
            abs_alliance_can_messages::HostBatteryRequestHostStateRequest::None => (
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default().bg(ratatui::style::Color::Green),
            ),
            _ => (
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
                ratatui::style::Style::default(),
            ),
        };

        text.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled("Mode: ", ratatui::style::Style::default()),
            ratatui::text::Span::styled("Sleep", sleep_style),
            ratatui::text::Span::styled(" ", ratatui::style::Style::default()),
            ratatui::text::Span::styled("Charge", charge_style),
            ratatui::text::Span::styled(" ", ratatui::style::Style::default()),
            ratatui::text::Span::styled("Drive", drive_style),
            ratatui::text::Span::styled(" ", ratatui::style::Style::default()),
            ratatui::text::Span::styled("None", none_style),
        ]));

        text.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!(
                    "Charge modules: {:012b} connected, {:012b} standby",
                    self.pack_diagnostic_connect
                        .num_modules_connected_for_charge,
                    self.pack_diagnostic_connect.num_modules_standby_for_charge
                ),
                ratatui::style::Style::default().fg(ratatui::style::Color::Black),
            ),
        ]));

        text.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!(
                    "Drive modules:  {:012b} connected, {:012b} standby",
                    self.pack_diagnostic_connect.num_modules_connected_for_drive,
                    self.pack_diagnostic_connect.num_modules_standby_for_drive
                ),
                ratatui::style::Style::default().fg(ratatui::style::Color::Black),
            ),
        ]));

        text.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!("SOC: {:.1}%", self.pack_soc.soc),
                ratatui::style::Style::default().fg(ratatui::style::Color::Black),
            ),
        ]));

        text.push(ratatui::text::Line::from(vec![
            ratatui::text::Span::styled(
                format!(
                    "{:.3}V {:8.3}A",
                    self.pack_hv_status.voltage, self.pack_hv_status.current,
                ),
                ratatui::style::Style::default().fg(ratatui::style::Color::Black),
            ),
        ]));

        ratatui::widgets::Paragraph::new(text)
            .block(
                ratatui::widgets::Block::new()
                    .title("Battery Pack")
                    .borders(ratatui::widgets::Borders::ALL)
                    .padding(ratatui::widgets::block::Padding::ZERO),
            )
            .render(area, buf);
    }
}

impl ratatui::widgets::Widget for &App {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let title = ratatui::text::Line::from(" ABS Alliance Battery Monitor ".bold());
        let title_bottom = ratatui::text::Line::from(vec![
            " ".into(),
            "Q".blue().bold(),
            "uit ".into(),
            "S".blue().bold(),
            "leep ".into(),
            "C".blue().bold(),
            "harge ".into(),
            "D".blue().bold(),
            "rive ".into(),
            "N".blue().bold(),
            "one ".into(),
        ]);

        let block = ratatui::widgets::Block::bordered()
            .title(title.centered())
            .title_bottom(title_bottom.centered())
            .border_set(ratatui::symbols::border::THICK);

        let layout = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints(vec![
                ratatui::layout::Constraint::Min(12),
                ratatui::layout::Constraint::Percentage(100),
            ])
            .split(block.inner(area));

        block.render(area, buf);

        self.battery_pack.render(layout[0], buf);

        // Convert slice of BatteryModule to Vec<ListItem>
        let items: Vec<ratatui::widgets::ListItem> = self
            .battery_pack
            .modules
            .iter()
            .map(|battery_module| {
                let mut text: Vec<ratatui::text::Line> = vec![];
                match battery_module.last_seen {
                    None => {
                        text.push(ratatui::text::Line::from(vec![
                        ratatui::text::Span::styled(
                            format!("Module absent"),
                            ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray),
                        )
                    ]));
                    }
                    Some(_) => {
                        text.push(
                            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                            format!(
                                "Serial {}",
                                battery_module.serial_number,
                            ),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Black),
                        )]));

                        text.push(
                            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                            format!(
                                "    SOC:{:5.1}% {:.3}V {:8.3}A SoH:{:5.1}% ({:.3}V {:.3}V {:.3}V)",
                                battery_module.soc.soc,
                                battery_module.hv_status.voltage,
                                battery_module.hv_status.current,
                                battery_module.soc.soh,
                                battery_module.adc2.rail_5v,
                                battery_module.adc2.rail_12v,
                                battery_module.adc2.rail_3v3,
                            ),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Black),
                        )]));

                        let mut battery_voltages: Vec<ratatui::text::Span> = vec![];
                        battery_voltages.push(ratatui::text::Span::styled(
                            "    ",
                            ratatui::style::Style::default()
                        ));

                        for i in 0..14 {
                            let mut style = ratatui::style::Style::default();
                            if battery_module.v_bricks[i] < 3.0 || battery_module.v_bricks[i] > 4.0 {
                                style = style.bg(ratatui::style::Color::Red);
                            }
                            if battery_module.balancing[i] {
                                style = style.fg(ratatui::style::Color::Blue);
                            }
                            battery_voltages.push(ratatui::text::Span::styled(
                                format!("{:5.3}", battery_module.v_bricks[i]),
                                style
                            ));
                            battery_voltages.push(ratatui::text::Span::styled(
                                " ",
                                ratatui::style::Style::default(),
                            ));
                        }

                        text.push(ratatui::text::Line::from( battery_voltages));

                        text.push(
                            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                            format!(
                                "    Temperatures: ambient:{:.1}°C module1:{:.1}°C module2:{:.1}°C FET:{:.1}°C Shunt:{:.1}°C",
                                battery_module.temperatures_a.ambient,
                                battery_module.temperatures_a.module1,
                                battery_module.temperatures_a.module2,
                                battery_module.temperatures_b.fet,
                                battery_module.temperatures_b.shunt,
                            ),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Black),
                        )]));

                        text.push(
                            ratatui::text::Line::from(vec![ratatui::text::Span::styled(
                            format!(
                                "    Charge request: {:.3}V {:8.3}A",
                                battery_module.charge_limit.voltage,
                                battery_module.charge_limit.current,
                            ),
                            ratatui::style::Style::default().fg(ratatui::style::Color::Black),
                        )]));
                    }
                }
                ratatui::widgets::ListItem::new(text)
            })
            .collect();

        // Create a List widget
        ratatui::widgets::List::new(items)
            .block(
                ratatui::widgets::Block::default()
                    .title("Battery Modules")
                    .borders(ratatui::widgets::Borders::ALL),
            )
            .highlight_style(ratatui::style::Style::default().bg(ratatui::style::Color::DarkGray))
            .render(layout[1], buf);
    }
}
