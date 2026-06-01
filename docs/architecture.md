THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Critical Zoomer Architecture Plan

## Definitions

Work: The hoarded unit; the results of iterations up till the point is determind as in the set or outside the set.

## Assemblies

### Headgroup

The headgroup is responsible for all things which run strictly at window framerate and face the user.
It ensures that the user sees what they expect immediately, even if the work really hasn't quite caught up.
It also contains all settings and IO.

The Headgroup sends the viewport position, factor of zoom, and screen size to the workgroup whenever any of those change.
The Headgroup sends the settings to the other two groups anytime they change.

### Workgroup

The workgroup is responsible for completing and hoarding work.
It immediately discards work which is no longer present in the viewport.

The Workgroup sends hoarded work to the Shadegroup.

### Shadegroup

The shadegroup is responsible for cosmetic layers generated from work.

The Shadegroup sends RGB pixels and their location to the Headgroup.

## Technologies

### Rust

### Steady State

### Egui

### Rug

## Requirements Allocation

Form Factor: Headgroup
System Policy: All
Control Scheme: Headgroup
Display Scheme: Headgroup
Cosmetic Options: Headgroup & Shadegroup
Controls Mechanics: Headgroup
Seamless: Headgroup & Workgroup
Deep: All
Tenacious: Workgroup
Hoarding: Workgroup
Fast: All
