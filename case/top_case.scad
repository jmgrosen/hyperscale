// Copyright (C) Jessie Grosen 2024
// SPDX-License-Identifier: CERN-OHL-P-2.0

include <BOSL2/std.scad>
include <BOSL2/rounding.scad>

include <common.scad>

/*
  ALL MEASUREMENTS IN MM
 */

/*
  TODO:
 */

PREVIEW = false;
PROTOTYPING = true;

TOP_CASE_HEIGHT = 9;

BUTTON_POS = [113.07, -54.4498];
BUTTON_RADIUS = 4;
ENCODER_POS = [135.64, -54.4498];
ENCODER_RADIUS = 5;
DISPLAY = translate([150.644, -41.8908], p=rect([58.78, 25.5], anchor=TOP+LEFT));
USB_SLOT = translate([209.4240, -54.5598], p=rect([10, 12]));
USB_SLOT_HEIGHT = 3;

difference() {
  offset_sweep(offset(top_flange, r=WALL_WIDTH, closed=true, check_valid=false),
	       height=TOP_CASE_HEIGHT, check_valid=false, steps=22,
               bottom=os_teardrop(r=2), top=os_circle(r=1));
  up(BOTTOM_HEIGHT)
    offset_sweep(top_flange, height=TOP_CASE_HEIGHT-BOTTOM_HEIGHT, check_valid=false, steps=22,
		 bottom=os_circle(r=2), top=os_circle(r=-1, extra=1));
  xflip()
    translate(PCB_OFFSET) union() {
      translate(concat(BUTTON_POS, [-BOTTOM_HEIGHT]))
        cylinder(h=3*BOTTOM_HEIGHT, r=BUTTON_RADIUS, $fn=80);
      translate(concat(ENCODER_POS, [-BOTTOM_HEIGHT]))
        cylinder(h=3*BOTTOM_HEIGHT, r=ENCODER_RADIUS, $fn=80);
      translate([0, 0, -BOTTOM_HEIGHT])
	linear_extrude(3*BOTTOM_HEIGHT)
	  polygon(DISPLAY);
      translate([0, 0, USB_SLOT_HEIGHT])
	linear_extrude(TOP_CASE_HEIGHT)
	  polygon(USB_SLOT);
    }
}
/*
linear_extrude(height=TOP_CASE_HEIGHT) offset_stroke(top_flange, width=[0, WALL_WIDTH], closed=true);
linear_extrude(height=BOTTOM_HEIGHT) region(top_flange);
*/
