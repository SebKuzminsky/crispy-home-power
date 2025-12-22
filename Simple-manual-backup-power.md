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
back. Charge whenever the AC is plugged in.


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


## Phase 1 - In progress

- Teensy
- Teensy powered from HV bus via Buck90 set to output 5V
- CAN to battery only, no automatic charger control
- Momentary push button to turn on: short Battery Ign to Bat-, ignites one module, then the others wake by CAN
- Use the same momentary push button to turn off: short a GPIO to Gnd
- USB for debugging (FIXME: awkward, teensy doesn't like simultaneous USB and external power, use caution)
- Use existing 48v to USB 30W adapters

The battery ignites if i connect Bat Ign to a Teensy GPIO, even if the
teensy is powered by the Buck90 and everything is powered down. Looks
like there's some coupling between the GPIOs and Gnd on the powered-off
MCU, so i'm using a diode to prevent current flowing into the gpio.


### Power

How to power the teensy?

USB is an option but then I don't get debugging. A dedicated 5V buck would be better
- https://hackaday.io/project/175733-6-50v-input-5v-output-buck
- https://www.electronics-lab.com/project/50v-5v-7a-synchronous-buck-step-converter/
- Buck90

I'm using a Buck90 DC-DC converter, with Vin connected to the battery
HV terminals, the pot dialed so Vout is 5V, and Vout connected to Teensy
Vin. This works well as long as i dont connect a computer to the Teensy
USB port.

There's a pad on the underside of the Teensy that i must cut to power
it by Vin and still use USB.

Alternatively, put a "kill switch" in the power line between the buck
and the teensy.


## Future work

add CAN to charger, automatic charging control

add plenty of 12v for powering car cig outlets

add an inverter - Grid-forming (stand-alone, not connected to the grid
or to other inverters) is fine for this application.
