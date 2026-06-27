# Introduction

This is a simple backup power system, basically a big USB battery.

The main use case is providing a bunch of USB chargers during brief
(1-2 day) power outages and car camping trips.


## Goals


### Requirements

1. Several (5+?) USB-PD outputs.

2. At least one USB output powerful enough to charge my laptop and run
   my Pinecil (20V, 100W).

3. Simple to use.
    - easy to turn on/off
    - big E-stop button on top?
    - somewhat easy to charge


### Optional stretch goals

1. 1 kW of AC output.

2. A bunch of 12V car cigarette outlets, for plugging in more USB chargers
   and my NiteCore AA/AAA charger.

3. PV charging!


### Design considerations

I've found one 48V DC to USB adapter, and it's pretty low power, only 30W
(12V, 2.5A). This satisfies Requirement 1 but not 2.

It's easy to find 12V to 100W USB adapters, from the automobile market,
though i don't know about efficiency of this option?

So maybe make 300+ W of 12V, and run some of it to one or two of these
100W car USB chargers.

For charging: mount a DeltaQ inside, with the AC cord hanging out the
back. Charge whenever the AC is plugged in. FIXME: Or maybe mount the
charger outside the battery box, at a fixed charging station, so you
can move the battery around more easily?


## Components

* a bunch of ABS Alliance E48-2.0 batteries

* a Delta-Q charger

* a little embedded system to control it via CAN and provide telemetry
  via a little screen or a web page or some LEDs

* a plywood enclosure that can be moved around with a hand truck

* some kind of inverter (FIXME, optional)


# Development phases


## Phase 0 - Done

Run battery and charger from laptop, each on a dedicated CANBUS, manual
software. Proof of concept, but awkward to use.


## Phase 1 - Done

- Teensy controls battery
- Teensy powered from HV bus via Buck90 set to output 5V
- CAN to battery only, no automatic charger control
- Momentary push button to turn on: short Battery Ign to Bat-, ignites one module, then the others wake by CAN
- Use the same momentary push button to turn off: short a GPIO to Gnd
- USB for debugging (FIXME: awkward, teensy doesn't like simultaneous USB and external power, use caution)
- Use existing 48v to USB 30W adapters (fused)

The battery ignites if i connect Bat Ign to a Teensy GPIO, even if the
teensy is powered by the Buck90 and everything is powered down. Looks
like there's some coupling between the GPIOs and Gnd on the powered-off
MCU, so i'm using a diode to prevent current flowing into the gpio.

I'm powering the Teensy using a Buck90 DC-DC converter, with Vin
connected to the battery HV terminals, the pot dialed so Vout is 5V,
and Vout connected to Teensy Vin. This works well as long as i dont
connect a computer to the Teensy USB port.

There's a pad on the underside of the Teensy that you can cut to power
it by Vin and still use USB, but i don't want to modify the Teensy and i
want to be able to still power it from USB, so i can program the Teensy
without turning on the battery.  I think this means i should put a kill
switch in the power line between the buck and the teensy, and remember
to use it wheneve i plug in USB.


## Phase 2 - In progress

### Done
- Add 240W 48V-to-12V buck
- Add 12V power distribution
- Add a bunch of 12v car outlets (each individually fused)
- Add 100W USB PD outlets

### To do
- Build an enclosure
    - Done
        - 11.85 mm thick birch plywood
        - ~2800 mm/min cutting feed for 1/2" 2 fl Carbide, 7 mm doc, slotting
        - use rough and finishing cuts
        - predrill, don't just countersink, and use holes for work holding
        - front is upside down in CAM
        - update countersink diameters
        - shelves too small for battery :-(
            - front-to-back: 241
        - front window too small
    - To do
        - sigh, add 25 mm for the factory battery LV connectors or slightly more for the digikey ones
        - the shelves should sit in a little slot in the front/back/sides
          to help align everything during assembly, cut slightly oversized
          because i'm using a larger diameter endmill
        - my clamps are too small, need 16" jaws at least, 4x, hackspace has them
        - front should have 5x holes for shelf, back has 2 like now
            - cutout on shelf towards rear
- add some kind of charging support
    - charger is heavy and bulky, should live outside the battery box
      and be connected only as needed
    - add 48V 20A panel-mount input from charger
        - Anderson SBxxx (available in 50-350 Amp, 50 A/2.5 kW is probably good), 6 or 8 AWG wire
        - anderson powerpole 15/45, 10 AWG wire
            - Blue to indicate 48 V
            - <https://www.printables.com/model/1355413-anderson-powerpole-panel-mount>
    - how to control charging?
        - add a CAN connection from the teensy to charger, let the teensy control charging?
        - add a CAN connection from battery to charger, let the batteries control charging?
        - control charging from laptop?
    - i think it's *possible* to charge while in Discharge mode,
      but i think it'll charge the highest-voltage modules, not the
      lowest-voltage ones
    - add some kind of UI to initiate charging?
        - keep the on/off button as it is now
        - add a SPDT toggle-switch to select Discharge or Charge mode
            - each throw connect a different GPIO to ground so the MCU knows
        - Or, MCU detects the charger via heartbeat on second MCU CAN
          input, switches automatically to charge mode?
    - DB9 for CAN between Charger and Teensy
    - need some indicator to user when charging's done
- Move from breadboard to something more permanent
    - better LV wiring, got solder cup connectors from digikey

### Maybe do
- Add power indicator LEDs (48V and 12V)
- Add 48V power distribution?
    - add panel mount Anderson PP or SB connectors for HV bus, usable by charger and inverter etc
- add a little screen showing Charge/Discharge mode, voltage, current, soc


## Future work

Add an inverter. Grid-forming (stand-alone, not connected to the grid
or to other inverters) is fine for this application. 120 VAC, 1300 W
_continuous_ would run each of my fridges & freezers in turn, and a
beefy electric heater when needed.
