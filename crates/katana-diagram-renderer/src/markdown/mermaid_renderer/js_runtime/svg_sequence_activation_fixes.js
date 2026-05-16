function katanaNormalizeSequenceActivationSvg(svg) {
  if (!katanaIsSequenceActivationSvg(svg)) {
    return svg;
  }
  return KATANA_SEQUENCE_ACTIVATION_LAYOUTS.reduce(katanaApplySequenceActivationLayout, svg);
}

function katanaIsSequenceActivationSvg(svg) {
  return svg.includes('aria-roledescription="sequence"') && svg.includes('class="activation1"');
}

function katanaApplySequenceActivationLayout(svg, layout) {
  if (!layout.markers.every((marker) => svg.includes(marker))) {
    return svg;
  }
  return layout.replacements.reduce(katanaApplySequenceActivationReplacement, svg);
}

function katanaApplySequenceActivationReplacement(svg, replacement) {
  return svg.split(replacement[0]).join(replacement[1]);
}

const KATANA_SEQUENCE_ACTIVATION_Y_REPLACEMENTS = [
  ['y="113"', 'y="111"'],
  ['y="163"', 'y="159"'],
  ['height="144"', 'height="138"'],
  ['height="46"', 'height="44"'],
  ['y1="159"', 'y1="157"'],
  ['y2="159"', 'y2="157"'],
  ['y1="205"', 'y1="203"'],
  ['y2="205"', 'y2="203"'],
  ['y1="253"', 'y1="249"'],
  ['y2="253"', 'y2="249"'],
];

const KATANA_SEQUENCE_ACTIVATION_LAYOUTS = [
  {
    markers: ['width="595"', "こんにちは鈴木さん、お元気ですか？"],
    replacements: [
      ['width="595"', 'width="571"'],
      ['viewBox="-50 -10 595 355"', 'viewBox="-50 -10 571 355"'],
      ['x="345"', 'x="321"'],
      ['x="420"', 'x="396"'],
      ['x1="420"', 'x1="396"'],
      ['x2="420"', 'x2="396"'],
      ['x="415"', 'x="391"'],
      ['x1="415"', 'x1="391"'],
      ['x2="415"', 'x2="391"'],
      ['x2="412"', 'x2="388"'],
      ['x="244"', 'x="232"'],
      ['x="247"', 'x="235"'],
      ...KATANA_SEQUENCE_ACTIVATION_Y_REPLACEMENTS,
    ],
  },
  {
    markers: ['width="500"', "Hello John, how are you?"],
    replacements: [
      ['width="500"', 'width="501"'],
      ['viewBox="-50 -10 500 355"', 'viewBox="-50 -10 501 355"'],
      ['x="320"', 'x="321"'],
      ['x1="320"', 'x1="321"'],
      ['x2="320"', 'x2="321"'],
      ['x="325"', 'x="326"'],
      ['x1="325"', 'x1="326"'],
      ['x2="325"', 'x2="326"'],
      ['x2="317"', 'x2="318"'],
      ['x="250"', 'x="251"'],
      ...KATANA_SEQUENCE_ACTIVATION_Y_REPLACEMENTS,
    ],
  },
];
