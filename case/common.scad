FRONT_BOTTOM_WIDTH = 117;
ROUNDING_START_WIDTH = 115;
BACK_ROUNDING_RADIUS = 15;
TOTAL_DEPTH=35.5;
BOTTOM_HEIGHT = 1.5;
WALL_WIDTH = 1;
TOP_CENTER_HEIGHT = 10;
PCB_CENTER_HEIGHT = 5;
TILT_ANGLE = 10;

PCB_OFFSET = [-144, 52.3];
MOUNT_POINTS = [
  [101.854, -45.974],
  // [145.796, -45.974],
  [145.796, -63.246],
  [184.324, -54.356],
];
MOUNT_POST_RADIUS = 1.4;
MOUNT_POST_HEIGHT = 3;

WIRE_HOLE_DIAMETER = 4;
WIRE_HOLE_HEIGHT = 5.5;
WIRE_TO_MAGNET_DIST = 11.25;
MAGNET_DEPTH = 3;
MAGNET_DIAMETER = 8;
MAGNET_HEIGHT = 5.75;
MAGNET_WALL_DEPTH_OFFSET = 2.5;

EXTRA_WIDTH_PER_SIDE = 9;
FLANGE_HEIGHT = 4;

// in PCB coordinates
FLANGE_CUTOUTS = [
  translate([209.42 - 9.5, -41.81 - 8.5], p=rect([6, 8.5], anchor=TOP+LEFT)), // battery connector: 5x8 at x=-9, y=-9
  translate([209.42 - 5.5, -41.81 - 17.8], p=rect([5.4, 7.3], anchor=TOP+LEFT)), // qwiic connector
  translate([209.42 - 12.5, -41.81 - 22.3], p=rect([6, 4], anchor=TOP+LEFT)), // reset button
];

BEND_RADIUS = 5;
STRAIN_RELIEF_WIDTH = 1;
PCB_SUPPORTS = [
  translate([97, -45], p=rect([23, 25])),
  translate([105, -65], p=rect([7, 25])),
  translate([148, -65], p=rect([15, 13])),
  translate([184, -57], p=rect([60, 14])),

  // translate([144, -50], p=circle(d=4, $fn=20)),
];

top_flange = round_corners(rect([FRONT_BOTTOM_WIDTH + EXTRA_WIDTH_PER_SIDE*2, TOTAL_DEPTH + WALL_WIDTH/2]), radius=2, $fn=20);
