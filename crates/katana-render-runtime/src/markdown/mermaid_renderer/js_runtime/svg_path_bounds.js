function katanaSerializedSvgPathBox(tag, offset) {
  return katanaOffsetBox(katanaSerializedPathDataBox(katanaAttrText(tag, "d")), offset);
}

function katanaSerializedPathDataBox(pathData) {
  return pathData ? katanaNonEmptySerializedPathDataBox(pathData) : null;
}

function katanaNonEmptySerializedPathDataBox(pathData) {
  return (
    katanaCurvedSerializedPathDataBox(pathData) ??
    katanaParsedSerializedPathDataBox(pathData) ??
    katanaNumberPairsBox(katanaSerializedSvgNumberList(pathData))
  );
}

function katanaCurvedSerializedPathDataBox(pathData) {
  return katanaIshikawaHeadPathBox(pathData);
}

function katanaIshikawaHeadPathBox(pathData) {
  const match = pathData.match(/^M 0 -?([\d.]+) L 0 ([\d.]+) Q ([\d.]+) 0 0 -?[\d.]+ Z$/);
  if (!match) {
    return null;
  }
  const halfHeight = Math.max(Number(match[1]), Number(match[2]));
  return [0, -halfHeight, Number(match[3]) / 2, halfHeight];
}

function katanaOffsetNumberListBox(value, offset) {
  return katanaOffsetBox(katanaNumberPairsBox(katanaSerializedSvgNumberList(value)), offset);
}

function katanaParsedSerializedPathDataBox(pathData) {
  const parser = katanaPathParser(pathData);
  while (parser.index < parser.tokens.length) {
    katanaReadPathCommand(parser);
  }
  return katanaNumberPairsBox(parser.points);
}

function katanaPathParser(pathData) {
  return {
    tokens: Array.from(String(pathData).matchAll(/[a-zA-Z]|-?(?:\d*\.)?\d+(?:e-?\d+)?/gi)).map(
      (it) => it[0],
    ),
    index: 0,
    command: "",
    x: 0,
    y: 0,
    startX: 0,
    startY: 0,
    curveType: null,
    curveControl: null,
    points: [],
  };
}

function katanaReadPathCommand(parser) {
  parser.command = katanaNextPathCommand(parser);
  const command = parser.command.toUpperCase();
  const relative = parser.command !== command;
  const reader = KATANA_PATH_COMMAND_READERS[command];
  if (reader) {
    reader(parser, relative);
    return;
  }
  parser.index += 1;
}

function katanaNextPathCommand(parser) {
  if (katanaIsPathCommand(parser.tokens[parser.index])) {
    return parser.tokens[parser.index++];
  }
  return parser.command;
}

function katanaIsPathCommand(token) {
  return /^[a-zA-Z]$/.test(String(token ?? ""));
}

const KATANA_PATH_COMMAND_READERS = {
  A: (parser, relative) => katanaReadPathArcs(parser, relative),
  C: (parser, relative) => katanaReadPathCubics(parser, relative, false),
  H: (parser, relative) => katanaReadPathHorizontalLines(parser, relative),
  L: (parser, relative) => katanaReadPathCoordinateGroups(parser, relative, 1),
  M: (parser, relative) => katanaReadPathMoves(parser, relative),
  Q: (parser, relative) => katanaReadPathQuadratics(parser, relative, false),
  S: (parser, relative) => katanaReadPathCubics(parser, relative, true),
  T: (parser, relative) => katanaReadPathQuadratics(parser, relative, true),
  V: (parser, relative) => katanaReadPathVerticalLines(parser, relative),
  Z: (parser) => katanaClosePath(parser),
};

function katanaReadPathQuadratics(parser, relative, smooth) {
  const numberCount = smooth ? 2 : 4;
  while (katanaCanReadPathNumbers(parser, numberCount)) {
    katanaReadPathQuadraticSegment(parser, relative, smooth);
  }
}

function katanaReadPathQuadraticSegment(parser, relative, smooth) {
  const origin = { x: parser.x, y: parser.y };
  const start = [origin.x, origin.y];
  const control = smooth
    ? katanaReflectedPathCurveControl(parser, start, "quadratic")
    : katanaReadPathPointFromOrigin(parser, relative, origin);
  const end = katanaReadPathPointFromOrigin(parser, relative, origin);
  katanaAddQuadraticBezierExtrema(parser.points, start, control, end);
  katanaSetPathPoint(parser, end);
  parser.curveType = "quadratic";
  parser.curveControl = control;
}

function katanaReadPathCubics(parser, relative, smooth) {
  const numberCount = smooth ? 4 : 6;
  while (katanaCanReadPathNumbers(parser, numberCount)) {
    katanaReadPathCubicSegment(parser, relative, smooth);
  }
}

function katanaReadPathCubicSegment(parser, relative, smooth) {
  const origin = { x: parser.x, y: parser.y };
  const start = [origin.x, origin.y];
  const first = smooth
    ? katanaReflectedPathCurveControl(parser, start, "cubic")
    : katanaReadPathPointFromOrigin(parser, relative, origin);
  const second = katanaReadPathPointFromOrigin(parser, relative, origin);
  const end = katanaReadPathPointFromOrigin(parser, relative, origin);
  katanaAddCubicBezierExtrema(parser.points, start, first, second, end);
  katanaSetPathPoint(parser, end);
  parser.curveType = "cubic";
  parser.curveControl = second;
}

function katanaReflectedPathCurveControl(parser, start, curveType) {
  if (parser.curveType !== curveType || !parser.curveControl) {
    return start;
  }
  return start.map((coordinate, axis) => 2 * coordinate - parser.curveControl[axis]);
}

function katanaReadPathMoves(parser, relative) {
  let firstMove = true;
  while (katanaCanReadPathNumbers(parser, 2)) {
    katanaSetPathPoint(parser, katanaReadPathPoint(parser, relative));
    if (firstMove) {
      parser.startX = parser.x;
      parser.startY = parser.y;
      parser.command = relative ? "l" : "L";
      firstMove = false;
    }
  }
}

function katanaReadPathCoordinateGroups(parser, relative, pointCount) {
  const numberCount = pointCount * 2;
  KATANA_PATH_COORDINATE_GROUP_READERS[Number(katanaCanReadPathNumbers(parser, numberCount))](
    parser,
    relative,
    pointCount,
    numberCount,
  );
}

function katanaReadPathPointGroup(parser, relative, pointCount) {
  const origin = { x: parser.x, y: parser.y };
  Array.from({ length: pointCount })
    .map(() => katanaReadPathPointFromOrigin(parser, relative, origin))
    .forEach((point) => katanaSetPathPoint(parser, point));
}

function katanaReadPathPointFromOrigin(parser, relative, origin) {
  const x = katanaReadPathNumber(parser);
  const y = katanaReadPathNumber(parser);
  return relative ? [origin.x + x, origin.y + y] : [x, y];
}

const KATANA_PATH_COORDINATE_GROUP_READERS = [
  () => {},
  (parser, relative, pointCount, numberCount) => {
    katanaReadPathPointGroup(parser, relative, pointCount);
    katanaReadPathCoordinateGroups(parser, relative, numberCount / 2);
  },
];

function katanaReadPathHorizontalLines(parser, relative) {
  while (katanaCanReadPathNumbers(parser, 1)) {
    katanaSetPathPoint(parser, [
      katanaPathCoordinate(parser, katanaReadPathNumber(parser), relative, "x"),
      parser.y,
    ]);
  }
}

function katanaReadPathVerticalLines(parser, relative) {
  while (katanaCanReadPathNumbers(parser, 1)) {
    katanaSetPathPoint(parser, [
      parser.x,
      katanaPathCoordinate(parser, katanaReadPathNumber(parser), relative, "y"),
    ]);
  }
}

function katanaReadPathArcs(parser, relative) {
  while (katanaCanReadPathNumbers(parser, 7)) {
    const start = [parser.x, parser.y];
    const radius = [Math.abs(katanaReadPathNumber(parser)), Math.abs(katanaReadPathNumber(parser))];
    const rotation = katanaReadPathNumber(parser);
    katanaReadPathNumber(parser);
    const sweep = katanaReadPathNumber(parser);
    const end = katanaReadPathPoint(parser, relative);
    katanaSetPathPoint(parser, end);
    katanaAddAxisAlignedArcExtremum(parser, start, end, radius, rotation, sweep);
  }
}

function katanaAddAxisAlignedArcExtremum(parser, start, end, radius, rotation, sweep) {
  const deltaX = end[0] - start[0];
  const deltaY = end[1] - start[1];
  if (rotation === 0 && deltaY === 0 && Math.abs(deltaX) === radius[0] * 2) {
    const direction = Math.sign(deltaX) * (sweep === 0 ? 1 : -1);
    parser.points.push((start[0] + end[0]) / 2, start[1] + direction * radius[1]);
    return;
  }
  if (rotation === 0 && deltaX === 0 && Math.abs(deltaY) === radius[1] * 2) {
    const direction = Math.sign(deltaY) * (sweep === 0 ? -1 : 1);
    parser.points.push(start[0] + direction * radius[0], (start[1] + end[1]) / 2);
  }
}

function katanaClosePath(parser) {
  katanaSetPathPoint(parser, [parser.startX, parser.startY]);
}

function katanaReadPathPoint(parser, relative) {
  return [
    katanaPathCoordinate(parser, katanaReadPathNumber(parser), relative, "x"),
    katanaPathCoordinate(parser, katanaReadPathNumber(parser), relative, "y"),
  ];
}

function katanaPathCoordinate(parser, value, relative, axis) {
  return relative ? parser[axis] + value : value;
}

function katanaSetPathPoint(parser, point) {
  parser.x = point[0];
  parser.y = point[1];
  parser.points.push(point[0], point[1]);
  parser.curveType = null;
  parser.curveControl = null;
}

function katanaCanReadPathNumbers(parser, count) {
  const nextTokens = parser.tokens.slice(parser.index, parser.index + count);
  return nextTokens.length === count && nextTokens.every((token) => !katanaIsPathCommand(token));
}

function katanaReadPathNumber(parser) {
  return Number(parser.tokens[parser.index++]);
}

function katanaSerializedSvgNumberList(value) {
  return Array.from(String(value).matchAll(/-?(?:\d*\.)?\d+(?:e-?\d+)?/gi)).map((it) =>
    Number(it[0]),
  );
}

function katanaNumberPairsBox(numbers) {
  if (numbers.length < 2) {
    return null;
  }
  const xValues = numbers.filter((_value, index) => index % 2 === 0);
  const yValues = numbers.filter((_value, index) => index % 2 === 1);
  return [Math.min(...xValues), Math.min(...yValues), Math.max(...xValues), Math.max(...yValues)];
}
