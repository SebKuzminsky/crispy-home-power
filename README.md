manufacturer: American Battery Solutions
product line: Alliance
model: E48-2.0


# Wire harness

Net      | Harness pin | Wire color
-------- | ----------- | ----------
Ign Low  | 2           | Blue
CAN+ In  | 10          | Orange
CAN- In  | 9           | Black
CAN- Out | 7           | Purple
CAN+ Out | 6           | Gray


# Misc

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
