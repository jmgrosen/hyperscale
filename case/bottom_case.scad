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


strain_relief_maze = union([
  translate([0, 0], p=rect([STRAIN_RELIEF_WIDTH, BEND_RADIUS+2*STRAIN_RELIEF_WIDTH], anchor=TOP+LEFT)),
  translate([0, -BEND_RADIUS-STRAIN_RELIEF_WIDTH], p=rect([BEND_RADIUS+2*STRAIN_RELIEF_WIDTH, STRAIN_RELIEF_WIDTH], anchor=TOP+LEFT)),
  translate([BEND_RADIUS+STRAIN_RELIEF_WIDTH, 0], p=rect([BEND_RADIUS+2*STRAIN_RELIEF_WIDTH, STRAIN_RELIEF_WIDTH], anchor=TOP+LEFT)),
  translate([2*BEND_RADIUS+2*STRAIN_RELIEF_WIDTH, 0], p=rect([STRAIN_RELIEF_WIDTH, BEND_RADIUS*2+3*STRAIN_RELIEF_WIDTH], anchor=TOP+LEFT)),
  translate([BEND_RADIUS+STRAIN_RELIEF_WIDTH, -2*BEND_RADIUS-2*STRAIN_RELIEF_WIDTH], p=rect([BEND_RADIUS+2*STRAIN_RELIEF_WIDTH, STRAIN_RELIEF_WIDTH], anchor=TOP+LEFT)),
]);

rounding_start_depth = TOTAL_DEPTH - BACK_ROUNDING_RADIUS;
back_bottom_width = ROUNDING_START_WIDTH - 2*BACK_ROUNDING_RADIUS;
diagonal_width = (FRONT_BOTTOM_WIDTH - ROUNDING_START_WIDTH) / 2;
magnet_wall_offset = WIRE_TO_MAGNET_DIST + MAGNET_DIAMETER/2 + WALL_WIDTH;
base = move([-FRONT_BOTTOM_WIDTH/2, -TOTAL_DEPTH/2, 0], turtle([
  "turn", 90,
  "xymove", [diagonal_width, rounding_start_depth],
  "arcright", BACK_ROUNDING_RADIUS, 90,
  "move", back_bottom_width/2 - magnet_wall_offset,
  "turn", -90,
  "move", MAGNET_WALL_DEPTH_OFFSET,
  "turn", 90,
  "move", magnet_wall_offset * 2,
  "turn", 90,
  "move", MAGNET_WALL_DEPTH_OFFSET,
  "turn", -90,
  "move", back_bottom_width/2 - magnet_wall_offset,
  "arcright", BACK_ROUNDING_RADIUS, 90,
  "xymove", [diagonal_width, -rounding_start_depth],
]));

module below_pcb() {
  half_of(xrot(TILT_ANGLE, p=DOWN), cp=[0, 0, PCB_CENTER_HEIGHT], s=FRONT_BOTTOM_WIDTH*3)
    linear_extrude(height=TOP_CENTER_HEIGHT*2)
      polygon(base);
}

linear_extrude(height=BOTTOM_HEIGHT) polygon(base);

tilt_height = (TOTAL_DEPTH / 2) * tan(TILT_ANGLE);
total_height = TOP_CENTER_HEIGHT + tilt_height;

// perimeter wall
difference() {
  half_of(xrot(TILT_ANGLE, p=DOWN), cp=[0, 0, TOP_CENTER_HEIGHT], s=FRONT_BOTTOM_WIDTH*3)
    linear_extrude(height=TOP_CENTER_HEIGHT*2)
      offset_stroke(base, width=[0, -WALL_WIDTH], closed=true);
  translate([0, TOTAL_DEPTH / 2 - MAGNET_WALL_DEPTH_OFFSET, WIRE_HOLE_HEIGHT]) union() {
    teardrop(WALL_WIDTH*4, d=WIRE_HOLE_DIAMETER, $fn=20);
    if (PROTOTYPING) {
      cube([WIRE_HOLE_DIAMETER, WALL_WIDTH*4, TOP_CENTER_HEIGHT*2], anchor=BOTTOM);
    }
  }
  translate([0, 0, TOP_CENTER_HEIGHT-0.01]) xrot(TILT_ANGLE)
    translate(concat(PCB_OFFSET, [-BOTTOM_HEIGHT])) union() {
    for (cutout = FLANGE_CUTOUTS) {
      linear_extrude(height=BOTTOM_HEIGHT*3) polygon(cutout);
    }
  }
}

magnet_slot_rect = rect([MAGNET_DIAMETER, MAGNET_DEPTH]);

// magnet slots and strain relief
intersection() {
  below_pcb();
  union() {
    translate([0, TOTAL_DEPTH/2 - MAGNET_DEPTH/2 - MAGNET_WALL_DEPTH_OFFSET - WALL_WIDTH, 0]) xcopies(WIRE_TO_MAGNET_DIST * 2) {
      difference() {
        linear_extrude(height=MAGNET_HEIGHT) polygon(magnet_slot_rect);
        translate([0, -MAGNET_DEPTH/2 - 1, MAGNET_HEIGHT])
          #cylinder(h=MAGNET_DEPTH+2, d=MAGNET_DIAMETER, orient=BACK, $fn=40);
      }
      linear_extrude(height=MAGNET_HEIGHT + MAGNET_DIAMETER/2 + 1)
        offset_stroke(magnet_slot_rect, width=[0, WALL_WIDTH], closed=true);
    }
    color("blue") linear_extrude(total_height) translate([-BEND_RADIUS/2-STRAIN_RELIEF_WIDTH, 6, 0]) region(strain_relief_maze);
  }
}

intersection() {
  linear_extrude(height=total_height) polygon(base);
  // xrot(TILT_ANGLE) translate(concat(PCB_OFFSET, [PCB_CENTER_HEIGHT]))
  translate([0, 0, PCB_CENTER_HEIGHT])
    xrot(TILT_ANGLE)
      translate(PCB_OFFSET)
    color("red") union() {
    for (support = PCB_SUPPORTS) {
      zflip()
        linear_extrude(total_height)
          #region(support);
    }
    for (mount = MOUNT_POINTS) {
      translate(concat(mount, [0]))
        cylinder(MOUNT_POST_HEIGHT, r=MOUNT_POST_RADIUS, anchor=BOTTOM, $fn=20);
    }
  }
}

magnet_slot = translate([0, TOTAL_DEPTH/2], rect([magnet_wall_offset*2, MAGNET_WALL_DEPTH_OFFSET*4]));

translate([0, 0, TOP_CENTER_HEIGHT-0.01]) xrot(TILT_ANGLE)
// xrot(TILT_ANGLE) translate([0, 0, TOP_CENTER_HEIGHT-0.01])
union() {
  difference() {
    linear_extrude(height=BOTTOM_HEIGHT) union() {
      // offset_stroke(base, width=[0, -WALL_WIDTH]);
      region(difference(offset(top_flange, r=WALL_WIDTH, closed=true), base));
      offset_stroke(move([-FRONT_BOTTOM_WIDTH/2, -TOTAL_DEPTH/2, 0], turtle([
	"turn", 90,
	"xymove", [diagonal_width, rounding_start_depth],
	"arcright", BACK_ROUNDING_RADIUS, 90,
      ])), width=[0, -WALL_WIDTH], end="round");
      offset_stroke(move([FRONT_BOTTOM_WIDTH/2, -TOTAL_DEPTH/2, 0], turtle([
	"turn", 90,
	"xymove", [-diagonal_width, rounding_start_depth],
	"arcleft", BACK_ROUNDING_RADIUS, 90,
      ])), width=[0, WALL_WIDTH], end="round");
    }
    translate([0, 0, -1]) linear_extrude(height=FLANGE_HEIGHT+2) polygon(magnet_slot);
    translate(concat(PCB_OFFSET, [-BOTTOM_HEIGHT])) union() {
      for (cutout = FLANGE_CUTOUTS) {
	linear_extrude(height=BOTTOM_HEIGHT*3) polygon(cutout);
      }
    }
  }
  difference() {
    linear_extrude(height=FLANGE_HEIGHT) offset_stroke(top_flange, width=[0, -WALL_WIDTH], closed=true);
    translate([0, 0, -1]) linear_extrude(height=FLANGE_HEIGHT+2) polygon(magnet_slot);
  }
}


if (PREVIEW) {
  translate([0, 0, PCB_CENTER_HEIGHT])
  xrot(TILT_ANGLE)
    translate(PCB_OFFSET)
      import("../pcb/scale_pcb.stl");
}
