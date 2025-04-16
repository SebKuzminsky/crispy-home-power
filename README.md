# Introduction

Project goals:
- When the PV system generates more energy than my house uses, store excess in batteries.
- When the house needs more energy that the PV system generates, supply from batteries.

Storing into batteries is easy, use a current-controlled charger to charge the battery bank.

How to supply from batteries?
- Large current-controlled micro-inverter?
- Multiple smaller micro-inverters, turn on/off as needed?


# Batteries

manufacturer: American Battery Solutions
product line: Alliance
model: E48-2.0


## Wire harness

Net      | Harness pin | Wire color
-------- | ----------- | ----------
Ign Low  | 2           | Blue
CAN+ In  | 10          | Orange
CAN- In  | 9           | Black
CAN- Out | 7           | Purple
CAN+ Out | 6           | Gray


## CAN

Lots of good info in the Alliance User Manual.

CAN ID 0x502: `HOST_BatteryRequest`
    Payload: `HOST_stateRequest`, 1 byte
        0x00: None
        0x01: Drive/Discharge (power output)
        0x02: Charge (power input)
        0x03: Sleep
    Send this packet at 1 Hz

`$ while true; do cansend can0 '502#02'; sleep 1; done`


## Misc

Short the `IGN Low` pin to `BATT-` i think, and if the pack has
enough charge to boot the internal bms it'll energize `BATT+`.

Nathaniel says: I had one pack never shutoff after I turned it on with
an ignition signal, which is the main reason I no longer want to use
the pack BMS.

Matt Quick
    @Wrench Monkey
    Here's the battery testing instructions:
    Connect modules with the short jumper cables
    Connect charger cables to any two terminals
    Snug bolts.  Dewalt auto thing works well.

    Plug in the CAN dongle (before opening Busmaster!)
    Open Busmaster
    Load the config (This only has to be done once.  Busmaster will remember the last config loaded)
    Press "Connect"

    ASSUME BATTERIES ARE ON AT THIS POINT

    Press "Transmit Window"
    Verify Transmit Window is configured as shown in pic
    Expand all messages by pressing the "plus box"
    Verify PackNumModsConfigured = 10
    Verify PackNumModsOnNetwork = 10
    Verify vPackLoad is above 45

    Plug in deltaQ charger
    Verify iPack/iPackFiltered are above 20 (pack is charging)

    Wait for packNumModsOnHvBus = 10
    This will depend on any modules that are out of balance.
    Once the low modules charge to meet the high modules all 10 will come online.


# Charger

Delta-Q
    - Model: ICL 1500-058v
    - Part number: 943-0016


# Inverters


## Grid-forming

Grid-forming inverters supply voltage and frequency support to the grid,
as well as power.  This is needed for black start (bringing up the grid
after an outage).  Different modes of operation, such as: droop control,
virtual synchronous machine, hierachichal control, etc.

- PowerFlex 755T from Rockwell Automation
- Sunny Island from SMA Solar Technology
- PowerStore from ABB
- GridMaster from Ideal Power


## Grid-following

Grid-following: supplies power, but must receive voltage and frequency from the grid.


## Misc info, open questions

Good info:
- <https://energycentral.com/c/iu/grid-forming-vs-grid-following>
- (terrible presentation) <https://www.youtube.com/watch?v=zWj21MMHPJc>
- <https://www.youtube.com/watch?v=RKQo9metcpU>

Some inverters can be both grid-forming and grid-following?

connecting inverters in parallel?  They must be grid-following, except
maybe one that "supplies voltage and frequency"?

Scenarios:
- grid tied
- grid tied system responds to grid disconnect
- island/off-grid (are these synonymous?)
- island reconnects to the grid
- black start

microgrids, islanded grids, large-scale grids

how does the enphase battery connect to the microgrid?  Does it go into the Combiner?


## Inverter Options

The internet thinks "Enphase uses microinverters on their battery systems. They just stack them together."

Enphase M215/M250: <https://enphase.com/store/microinverters/legacy/m250-microinverter-kit>

Enphase IQ8X-BATT
- <https://enphase.com/download/iq8x-bat-microinverter-data-sheet>
- Needs 52.5 VDC to start?

Enphase IQ8+
- runs on 16–58 V DC input

- SunnyBoy

- Northern Electric BDM-600X: <https://northernep.com/downloads/technical-sheet/BDM-600X-Microinverter.pdf>


# Links & info

DIY Solar folks brainstorming a system very similar to what i want:
<https://diysolarforum.com/threads/adding-an-ess-to-enphase-enlighten-system.42737/>

<https://diysolarforum.com/threads/using-solar-micro-inverters-with-batteries-instead-of-panels.8353/>

Enphase design guides: <https://enphase.com/installers/training/getting-started/design>
