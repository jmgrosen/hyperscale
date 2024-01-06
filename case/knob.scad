include <BOSL2/std.scad>
include <BOSL2/rounding.scad>

KNOB_RADIUS = 12.7;
KNOB_THICKNESS = 5;
SHAFT_INSET = 1;
SHAFT_OUTER_RADIUS = 4;
SHAFT_INNER_RADIUS = 3;
SHAFT_THICKNESS = 6;
NOTCH_THICKNESS = 5;
NOTCH_OFFSET = 1.5;

difference() {
  union() {
    cylinder(r=KNOB_RADIUS, h=KNOB_THICKNESS, $fn=160);
    translate([0, 0, KNOB_THICKNESS - SHAFT_INSET]) cylinder(r=SHAFT_OUTER_RADIUS, h=SHAFT_THICKNESS, $fn=80);
  }
  translate([0, 0, KNOB_THICKNESS - SHAFT_INSET]) difference() {
    cylinder(r=SHAFT_INNER_RADIUS, h=SHAFT_THICKNESS+1, $fn=80);
    translate([NOTCH_OFFSET, 0, 0])
      cube([3*SHAFT_OUTER_RADIUS, 3*SHAFT_OUTER_RADIUS, NOTCH_THICKNESS], anchor=LEFT+BOTTOM);
  }
}
