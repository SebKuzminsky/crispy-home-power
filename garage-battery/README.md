This project is made from five parts:
1. Varta batteries
2. Victon inverter
3. Delta-Q charger (AC powered)
4. Enphase PV system
5. A Pi running the software in this repo

The batteries power the inverter, and the inverter powers a couple
of extension cords in the garage. The load is a fridge and a freezer,
the draw is about 200 W to 400 W.


# Software

The software in this repo monitors the SoC of the Varta batteries and
tries to keep them charged, using excess power from the PV array ideally
but falling back to grid AC power when needed.


## Varta Monitor

The Varta monitor looks at the Varta Master info, specifically the state
of charge and the charge request current.


## PV Monitor

The PV Monitor reads data from the Enphase Envoy via local Wifi (no
clown computing). Specifically we're looking to see if the PV system is
over-generating so we're exporting power to the grid.


## Control logic

The controller looks at the information gathered by the Varta Monitor
and the PV Monitor.  If the PV system is over-generating, we try to
divert this power to the batteries.

The Controller manages some internal configuration variables:
* minimum SoC: Take action when the batteries get below this.
* max SoC: Do not charge the batteries above this.
* charge current: Try to charge the batteries at this rate.

The Control logic works like this:

- When the house is generating excess power we set min SoC to 75%, max
  SoC to 80%, and "available current" sufficient to absorb most of the
  exported power.

- When there is no power being exported we set a min SoC of 20%, max
  SoC of 25%, and "available current" to infinity.

If the battery SoC is above the max SoC, we request the Charger to turn off.

If the battery SoC is below the min SoC, we request the Charger to turn
on. The charge current is the minimum of "available current" computed
above an the Varta charge request current.


## Charge controller

The charge controller just drives the Delta-Q charger as specified by
the Control logic.
