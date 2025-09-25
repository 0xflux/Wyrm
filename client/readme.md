# Wyrm client

This is the CLI client for Wyrm.

## Interface

### Stale agents

If the agent has gone stale (you can modify these timeouts in your c2/.env file); it will appear in the primary 
panel as a grayed out entry. If you wish to remove a stale agent from your live list, you can select the agent and 
type `remove agent` into the console, removing the agent until it checks back in.

### Misc 

**Warning:** Issuing the `kill agent` command, this will kill the agent before any other pending tasks can be 
completed on the implant - **potentially** leading to the other pending tasks being lost. This will **not**
be the case for every task, but be aware of this.

## Colours

rgb(244, 219, 214)
rgb(240, 198, 198)
rgb(245, 189, 230)
rgb(198, 160, 246)
rgb(237, 135, 150)
rgb(238, 153, 160)
rgb(245, 169, 127)
rgb(238, 212, 159)
rgb(166, 218, 149)
rgb(139, 213, 202)
rgb(145, 215, 227)
rgb(125, 196, 228)
rgb(138, 173, 244)
rgb(183, 189, 248)

Text:
rgb(202, 211, 245)

Subtext:
rgb(73, 106, 236)
rgb(165, 173, 203)


Grays:
rgb(147, 154, 183)
rgb(128, 135, 162)
rgb(110, 115, 141)
rgb(91, 96, 120)
rgb(73, 77, 100)
rgb(54, 58, 79)
rgb(36, 39, 58)
rgb(30, 32, 48)
rgb(24, 25, 38)