function katanaAddQuadraticBezierExtrema(points, start, control, end) {
  for (let axis = 0; axis < 2; axis += 1) {
    const denominator = start[axis] - 2 * control[axis] + end[axis];
    if (denominator === 0) {
      continue;
    }
    const parameter = (start[axis] - control[axis]) / denominator;
    if (parameter > 0 && parameter < 1) {
      points.push(...katanaQuadraticBezierPoint(start, control, end, parameter));
    }
  }
}

function katanaQuadraticBezierPoint(start, control, end, parameter) {
  const inverse = 1 - parameter;
  return start.map(
    (coordinate, axis) =>
      inverse * inverse * coordinate +
      2 * inverse * parameter * control[axis] +
      parameter * parameter * end[axis],
  );
}

function katanaAddCubicBezierExtrema(points, start, first, second, end) {
  for (let axis = 0; axis < 2; axis += 1) {
    const parameters = katanaCubicBezierExtremaParameters(start, first, second, end, axis);
    parameters
      .filter((parameter) => parameter > 0 && parameter < 1)
      .forEach((parameter) => points.push(...katanaCubicBezierPoint(start, first, second, end, parameter)));
  }
}

function katanaCubicBezierExtremaParameters(start, first, second, end, axis) {
  const a = -start[axis] + 3 * first[axis] - 3 * second[axis] + end[axis];
  const b = 2 * (start[axis] - 2 * first[axis] + second[axis]);
  const c = first[axis] - start[axis];
  return katanaQuadraticRoots(a, b, c);
}

function katanaQuadraticRoots(a, b, c) {
  if (a === 0) {
    return b === 0 ? [] : [-c / b];
  }
  const discriminant = b * b - 4 * a * c;
  if (discriminant < 0) {
    return [];
  }
  if (discriminant === 0) {
    return [-b / (2 * a)];
  }
  const squareRoot = Math.sqrt(discriminant);
  const q = -0.5 * (b + (b >= 0 ? squareRoot : -squareRoot));
  return [q / a, c / q];
}

function katanaCubicBezierPoint(start, first, second, end, parameter) {
  const inverse = 1 - parameter;
  return start.map(
    (coordinate, axis) =>
      inverse ** 3 * coordinate +
      3 * inverse ** 2 * parameter * first[axis] +
      3 * inverse * parameter ** 2 * second[axis] +
      parameter ** 3 * end[axis],
  );
}
