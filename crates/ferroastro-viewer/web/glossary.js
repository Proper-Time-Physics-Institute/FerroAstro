// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// glossary.js: the viewer's explanatory text, in one place. Every hover/focus tooltip and every
// glossary entry is read from FA_GLOSSARY.entries, keyed by id, so the two never disagree.
// Display text only: no number here feeds the physics, the chart or the table. Values match
// docs/viewer/DESIGN.md section 8 and the engine sources (crates/ferroastro-viewer/src).
//
// Entry fields:
//   term     the name shown as the heading (required)
//   sym      an optional symbol or short form (ϖ̇, ″/cy, ...)
//   group    a key of FA_GLOSSARY.groups (required)
//   short    1-2 plain sentences for the tooltip (required)
//   typical  optional unit and typical value, shown in the tooltip and the entry
//   long     2-5 sentences for the glossary (required)
//   mercury  why it matters for Mercury's perihelion (required)
//   see      ids of related entries
// In `long` and `mercury`, [[id]] or [[id|text]] links to another entry; tooltips show the text.
//
// Loaded by index.html as a classic script (window.FA_GLOSSARY) and by check-glossary.mjs in
// node through vm (globalThis.FA_GLOSSARY), like fav.js.

(function (root) {
  "use strict";

  const groups = [
    { id: "orbits", title: "Bodies and orbits" },
    { id: "frames", title: "Frames and reference planes" },
    { id: "effects", title: "Effects (the physics switches)" },
    { id: "comparison", title: "Comparison models (outside the GR ladder)" },
    { id: "integrators", title: "Integrators and the engine" },
    { id: "time", title: "Time and units" },
    { id: "fit", title: "Fit and statistics" },
    { id: "table", title: "Contribution table" },
    { id: "data", title: "Data and provenance" },
    { id: "controls", title: "Viewer controls and display" },
  ];

  const E = {};

  // ------------------------------------------------------------------------------------ orbits
  E["mercury"] = {
    term: "Mercury", sym: "☿", group: "orbits",
    short: "The innermost planet and the subject of this page. Its orbit is a noticeably elongated ellipse whose orientation slowly turns.",
    typical: "a ≈ 0.387 au, e ≈ 0.206, period ≈ 87.97 days (≈ 415 orbits per century)",
    long: "Mercury goes round the Sun in about 88 days on an ellipse with [[eccentricity]] about 0.21, the largest of the planets, so its [[perihelion]] is well defined and easy to track. Because it is closest to the Sun it also feels the strongest relativistic correction. On the plate it is drawn in the ink colour with its [[osculating-ellipse]] and [[perihelion-markers]].",
    mercury: "Mercury is the classic test of gravity: its perihelion advances by about 575 ″/cy in an [[inertial-frame]], of which about 43 ″/cy is due to [[gr|general relativity]].",
    see: ["perihelion", "varpi-dot", "sun"],
  };
  E["sun"] = {
    term: "Sun", sym: "☉", group: "orbits",
    short: "The central body, drawn at the origin of the plate. Every position on the plate is measured from it (heliocentric).",
    typical: "GM☉ from DE440; radius about 696 000 km",
    long: "In System mode the Sun is one of the integrated bodies: it moves around the solar-system barycentre, and the plate shows everything relative to it ([[heliocentric]]). Its mass sets Mercury's Keplerian orbit; its slight flattening ([[solar-j2]]) and its rotation ([[lense-thirring]]) add tiny extra effects. In the [[rung-lab]] the Sun is fixed and Mercury is a [[test-particle]].",
    mercury: "The Sun's Newtonian pull alone gives a closed ellipse with no advance; everything this page measures comes from departures from that ([[kepler-problem]]).",
    see: ["heliocentric", "gm", "kepler-problem"],
  };
  E["perihelion"] = {
    term: "Perihelion", group: "orbits",
    short: "The point of an orbit closest to the Sun. Its direction is what this page tracks.",
    typical: "Mercury: about 0.307 au from the Sun (a(1−e))",
    long: "On an ellipse the perihelion is the end of the long axis nearest the focus where the Sun sits; the opposite end is the aphelion. Its direction in space is given by the [[varpi|longitude of perihelion]] ϖ. If the orbit were a pure two-body ellipse ([[kepler-problem]]) the perihelion would stay fixed for ever; perturbations make it turn slowly, which is the [[varpi-dot|perihelion advance]].",
    mercury: "Mercury's perihelion turns by about 1.4″ per orbit (≈ 575 ″/cy in the Park17 frame), far too little to see on the plate without the [[display-exaggeration]].",
    see: ["perihelion-passage", "varpi", "varpi-dot"],
  };
  E["perihelion-passage"] = {
    term: "Perihelion passage", group: "orbits",
    short: "A moment when Mercury is at its closest to the Sun. The engine records the direction of each passage; those directions are what the rate is fitted to.",
    typical: "about 4.15 passages per year; at least 20 (about 5 years) before a fit",
    long: "The engine samples Mercury's position and velocity relative to the Sun (every [[wh-step|WHFast step]], or every day with [[ias15|IAS15]]) and finds each minimum of the Sun distance by 8th-order Lagrange interpolation between samples (the Park2017 [[estimator]]). For each passage it stores the time and the [[projected-longitude]] of Mercury's position on the [[park17-plane]]. A passage is reported once its interpolation window is complete, so passages appear a few samples after the integration has passed them.",
    mercury: "Using actual passage directions, instead of the wiggly [[osculating-varpi]], is how Park et al. (2017) measure the advance; the viewer follows that definition (ADR-0011).",
    see: ["projected-longitude", "least-squares", "perihelion-markers"],
  };
  E["varpi"] = {
    term: "Longitude of perihelion", sym: "ϖ", group: "orbits",
    short: "The angle that gives the direction of the perihelion, ϖ = Ω + ω: measured along the reference plane to the orbit's node, then along the orbit to the perihelion.",
    typical: "degrees or arcseconds; the chart plots ϖ − ϖ₀ in arcseconds",
    long: "ϖ (\"varpi\", a variant of pi) is the sum of the longitude of the ascending node Ω, measured in the reference plane, and the argument of perihelion ω, measured in the orbit plane from that node. Because it is a \"dog-leg\" angle across two planes, its value depends on which reference plane is used: the viewer's canonical plane is the [[park17-plane]], and the [[ecliptic-j2000]] is offered for comparison ([[varpi-ecl]]). The chart shows ϖ relative to its value at the first passage, ϖ₀.",
    mercury: "The perihelion advance is simply the rate of change of ϖ, written [[varpi-dot|ϖ̇]].",
    see: ["varpi-dot", "osculating-varpi", "projected-longitude", "varpi-ecl"],
  };
  E["varpi-dot"] = {
    term: "Perihelion advance rate", sym: "ϖ̇", group: "orbits",
    short: "How fast the perihelion direction turns, from a least-squares fit to the passage longitudes. Shown ± the jack-knife uncertainty.",
    typical: "full model ≈ 575.31 ″/cy (Park17 plane); GR alone ≈ 42.98 ″/cy; Kepler only 0",
    long: "ϖ̇ (\"varpi dot\") is the time derivative of the [[varpi|longitude of perihelion]], in arcseconds per Julian century ([[arcsec-per-century|″/cy]]). The viewer obtains it by fitting the [[fit-model]] to the [[projected-longitude|projected passage longitudes]]; ϖ̇ is the slope at J2000.0, the fit's time origin (this matters only when [[fit-q|Q]] is included). The ± figure is the [[jackknife|jack-knife σ]]. Short runs give wobbly rates because the periodic perturbations have not averaged out; literature comparisons need 1000 years or more.",
    mercury: "In the Park17 frame the planets give ≈ 532.3 ″/cy, GR ≈ 42.98, solar J2 ≈ 0.03 and Lense–Thirring ≈ −0.002, about 575.31 ″/cy in total.",
    see: ["fit-model", "jackknife", "frame-caveat", "delta-varpi"],
  };
  E["delta-varpi"] = {
    term: "Advance so far", sym: "Δϖ", group: "orbits",
    short: "The latest passage longitude minus the first: how far the perihelion has actually turned since the run began.",
    typical: "with the full model about 5.75″ per year, 57.5″ per decade",
    long: "Δϖ is read straight from the passage buffer: the [[projected-longitude]] of the most recent [[perihelion-passage]] minus that of the first one. It is shown in words under the plate and as the labelled arc between the first and latest perihelion lines. Unlike [[varpi-dot|ϖ̇]] it is not fitted, so it still contains the periodic wiggles. When the [[display-exaggeration]] is above ×1 the arc is drawn k times wider, and its label says so.",
    mercury: "Over a single year Δϖ is only a few arcseconds and can even step backwards between passages; the steady trend shows up over decades.",
    see: ["varpi-dot", "perihelion-markers", "display-exaggeration"],
  };
  E["osculating-elements"] = {
    term: "Osculating elements", group: "orbits",
    short: "The ellipse Mercury would follow if every perturbation switched off at this instant, computed from its current position and velocity.",
    typical: "a, e, ϖ with μ = GM☉ + GM☿",
    long: "\"Osculating\" means \"kissing\": the osculating orbit touches the real path at one instant, with the same position and velocity. The engine computes [[semi-major-axis|a]], [[eccentricity|e]] and [[varpi|ϖ]] from Mercury's heliocentric position and velocity with the two-body gravitational parameter μ = GM☉ + GM☿. Because the planets keep tugging, the osculating elements oscillate around slowly drifting mean values.",
    mercury: "The osculating ϖ swings by tens of arcseconds over Mercury's [[synodic-period|synodic periods]], which is why the rate is fitted to passages, not to this curve.",
    see: ["osculating-ellipse", "osculating-varpi", "eccentricity", "semi-major-axis"],
  };
  E["osculating-ellipse"] = {
    term: "Osculating ellipse", group: "orbits",
    short: "The solid ellipse is Mercury's osculating orbit now; the faint dashed one is its osculating orbit at the start of the run.",
    long: "The plate draws the conic with the current [[semi-major-axis|a]], [[eccentricity|e]] and ϖ on the [[ecliptic-j2000|J2000 ecliptic]] (inclination neglected, so this is the top-down outline) and a dot at its perihelion. The ghost ellipse from the start of the run stays behind, so the gap between the two perihelia shows the advance. With the [[display-exaggeration]] above ×1 both ellipses are rotated by (k − 1)·Δϖ for drawing only.",
    mercury: "At ×1 the two ellipses overlap to within a line width even after a thousand years (the advance is about 1.6° per thousand years).",
    see: ["osculating-elements", "display-exaggeration", "perihelion-markers"],
  };
  E["osculating-varpi"] = {
    term: "Osculating ϖ", group: "orbits",
    short: "The longitude of perihelion of the instantaneous osculating ellipse, drawn as a thin line on the chart. It is drawn only, not fitted.",
    typical: "wiggles of tens of arcseconds",
    long: "This line is ϖ = Ω + ω of the [[osculating-elements|osculating orbit]] on the [[park17-plane]], sampled every [[display-cadence]] days (in the [[rung-lab]] it is the angle of the eccentricity vector projected on the initial orbit plane). It oscillates at the planets' [[synodic-period|synodic periods]] by far more than the secular trend over a few years. The fitted series is the [[projected-longitude]] of actual passages instead, which samples the orbit once per revolution at the same orbital phase.",
    mercury: "Switch on \"Subtract fitted rate\" ([[detrend]]) to see these short-period wiggles around the trend.",
    see: ["osculating-elements", "perihelion-passage", "varpi-ecl"],
  };
  E["eccentricity"] = {
    term: "Eccentricity", sym: "e", group: "orbits",
    short: "How elongated the orbit is: 0 is a circle, close to 1 a very long ellipse. The sparkline shows Mercury's osculating e over time.",
    typical: "Mercury ≈ 0.2056",
    long: "For an ellipse e = c/a, where c is the distance from the centre to the focus. The engine reports the [[osculating-elements|osculating]] e from Mercury's heliocentric state. The planets make it oscillate slightly over months and drift slowly over millennia; the number next to the sparkline is the latest value, and hovering the line shows its range.",
    mercury: "A larger e makes the perihelion direction sharper and easier to measure; the GR advance also depends on e through the factor 1/(1 − e²).",
    see: ["semi-major-axis", "osculating-elements"],
  };
  E["semi-major-axis"] = {
    term: "Semi-major axis", sym: "a", group: "orbits",
    short: "Half the long axis of the ellipse: the orbit's size. The sparkline shows Mercury's osculating a, in astronomical units.",
    typical: "Mercury ≈ 0.3871 au",
    long: "The semi-major axis fixes the orbital period through Kepler's third law. The engine reports the [[osculating-elements|osculating]] a with μ = GM☉ + GM☿. Planetary perturbations make it oscillate slightly, but there is no secular drift in a, so its line shows small oscillations around a constant.",
    mercury: "The GR advance per orbit scales as 1/(a(1 − e²)), which is why the innermost planet shows the largest relativistic effect.",
    see: ["eccentricity", "au", "osculating-elements"],
  };
  E["kepler-problem"] = {
    term: "Kepler problem", group: "orbits",
    short: "Two bodies under Newton's inverse-square gravity. The orbit is a fixed ellipse, so the perihelion does not move at all.",
    typical: "expected ϖ̇ = 0",
    long: "With only the Sun and Mercury and Newtonian gravity, the orbit closes exactly on itself every revolution (a special property of the 1/r² force). Any perihelion motion therefore needs an extra ingredient: other planets, relativity, the Sun's shape or spin. The \"Kepler only\" preset runs exactly this case, and the contribution table uses it as the [[integrator-floor]].",
    mercury: "Running Kepler only and seeing ϖ̇ ≈ 0 shows that the advance you see elsewhere comes from physics, not from the integrator.",
    see: ["preset-kepler", "integrator-floor", "newtonian-gravity"],
  };
  E["test-particle"] = {
    term: "Test particle", group: "orbits",
    short: "A body treated as massless: it feels the Sun's gravity but does not pull back. Used in the Rung lab.",
    long: "In the [[rung-lab]] Mercury moves in the field of a fixed Sun and nothing else. That is the setting in which the special-relativity rungs of ADR-0012 are defined ([[sr-a]], [[sr-b]]): their Hamiltonians contain only the Sun's potential. The engine label reads \"IAS15 (test particle)\".",
    mercury: "The test-particle problem isolates the central-force part of the advance (GR ≈ 42.98 ″/cy) from the planets' ≈ 532 ″/cy.",
    see: ["rung-lab", "rung", "ias15"],
  };
  E["synodic-period"] = {
    term: "Synodic period", group: "orbits",
    short: "The time between successive alignments of two planets as seen from the Sun. The planets' tugs on Mercury repeat with these periods.",
    typical: "Mercury–Venus ≈ 145 days; Mercury–Earth ≈ 116 days",
    long: "For two orbital periods P₁ < P₂ the synodic period is 1/(1/P₁ − 1/P₂). Gravitational kicks from a planet are strongest near conjunction, so the [[osculating-varpi]] oscillates with these periods and their combinations. The [[periodic-terms]] of the fit are built from the same mean motions.",
    mercury: "Fits over a few years catch these oscillations at random phases, which is why short-span rates wobble.",
    see: ["osculating-varpi", "periodic-terms"],
  };

  // ------------------------------------------------------------------------------------ frames
  E["inertial-frame"] = {
    term: "Inertial frame", group: "frames",
    short: "A reference frame that does not rotate or accelerate. The simulation's axes are inertial (ICRF), so no precession of the axes is mixed into the result.",
    long: "Newton's laws hold in their simple form only in an inertial frame. The engine integrates in [[icrf|ICRF]] axes, which are fixed relative to distant quasars. Astronomical coordinates referred to the moving [[equinox-of-date]] are not inertial: they rotate with the [[general-precession]] of the Earth's axis.",
    mercury: "This is why the viewer gives ≈ 575 ″/cy, not the ≈ 5600 ″/cy measured against the equinox of date (see the [[frame-caveat]]).",
    see: ["icrf", "frame-caveat", "general-precession"],
  };
  E["icrf"] = {
    term: "ICRF", group: "frames",
    short: "International Celestial Reference Frame: fixed, non-rotating axes defined by distant quasars. The engine integrates in ICRF axes.",
    long: "The ICRF is realised by the positions of hundreds of extragalactic radio sources and is aligned with the old J2000 equator and equinox to within a few hundredths of an arcsecond. JPL's DE440 ephemeris, and so the [[initial-conditions]], use ICRF axes. The viewer rotates positions into the [[ecliptic-j2000]] only for drawing, and measures longitudes on the [[park17-plane]], whose pole is given in ICRF coordinates.",
    mercury: "Because ICRF does not rotate, the advance measured here is purely dynamical.",
    see: ["inertial-frame", "ecliptic-j2000", "park17-plane"],
  };
  E["ecliptic-j2000"] = {
    term: "J2000 ecliptic", group: "frames",
    short: "The plane of the Earth's orbit, frozen at the epoch J2000.0. The plate is drawn top-down on this plane.",
    typical: "tilted 84381.448″ (≈ 23.44°) from the ICRF equator",
    long: "The viewer obtains ECLIPJ2000 coordinates by rotating ICRF about the x axis by the J2000 obliquity, 84381.448″. Because the plane is fixed it is an [[inertial-frame|inertial]] reference, unlike the ecliptic of date. The x axis points to the [[equinox-j2000]]. Mercury's orbit is tilted about 7° to this plane; the top-down view ignores that tilt.",
    mercury: "Measured on this plane the same physics gives ≈ 577.72 ″/cy instead of 575.31 ″/cy on the Park17 plane, because ϖ is a dog-leg angle that depends on the reference plane ([[varpi-ecl]]).",
    see: ["equinox-j2000", "park17-plane", "varpi-ecl"],
  };
  E["equinox-j2000"] = {
    term: "J2000 equinox", sym: "♈︎", group: "frames",
    short: "The fixed direction of the vernal equinox at J2000.0: the x axis of the ecliptic frame, drawn as the dashed line to the right of the Sun.",
    long: "The equinox is where the ecliptic crosses the celestial equator; ♈︎ (Aries) is its traditional symbol. The real equinox drifts by about 50″ per year because of the [[general-precession]], so the viewer uses the direction frozen at [[j2000|J2000.0]]. Longitudes on the plate are counted anticlockwise from this line.",
    mercury: "Against this fixed direction Mercury's perihelion turns about 578 ″/cy; against the moving equinox it would be about 5600 ″/cy.",
    see: ["ecliptic-j2000", "equinox-of-date", "general-precession"],
  };
  E["park17-plane"] = {
    term: "Park17 mean-orbit plane", group: "frames",
    short: "The fixed plane of Mercury's mean orbit used by Park et al. (2017). Perihelion longitudes are measured on it; it is the viewer's canonical frame (ADR-0011).",
    typical: "pole at ICRF RA 280.9876°, Dec 61.4481°",
    long: "Because ϖ is measured partly along a reference plane, its rate depends on which plane is chosen. Park et al. (2017) used a fixed plane close to Mercury's own orbit, which keeps the dog-leg distortion small, and ADR-0011 adopts it so the viewer's numbers compare directly with theirs. In System mode every passage longitude and the osculating ϖ on the chart are measured on this plane. (In the Rung lab the engine instead projects on the [[test-particle]]'s initial osculating orbit plane; its metadata names the plane.)",
    mercury: "The full-model total is ≈ 575.31 ″/cy on this plane, against ≈ 577.72 ″/cy on the fixed J2000 ecliptic.",
    see: ["projected-longitude", "park17", "ecliptic-j2000", "varpi"],
  };
  E["projected-longitude"] = {
    term: "Projected passage longitude", group: "frames",
    short: "The direction of Mercury's position at a perihelion passage, projected onto the Park17 plane and measured as an angle in that plane. This is what gets fitted.",
    typical: "arcseconds, unwrapped so it does not jump at 360°",
    long: "At each [[perihelion-passage]] the engine projects Mercury's heliocentric position onto the [[park17-plane]] and takes its angle from the plane's fixed reference direction. The series is unwrapped (whole turns added) so it rises smoothly. This is the ADR-0011 observable and the dots on the chart; the [[least-squares]] fit of these angles against time gives [[varpi-dot|ϖ̇]].",
    mercury: "Sampling the orbit once per revolution at perihelion removes most of the large short-period swing of the [[osculating-varpi]].",
    see: ["perihelion-passage", "park17-plane", "least-squares"],
  };
  E["varpi-ecl"] = {
    term: "ϖ on the J2000 ecliptic", group: "frames",
    short: "The osculating longitude of perihelion measured on the J2000 ecliptic instead of the Park17 plane. Drawn dashed, for comparison only.",
    typical: "with the full model its trend is about 2.4 ″/cy steeper than on the Park17 plane",
    long: "The same osculating orbit gives a different ϖ on a different reference plane, because ϖ = Ω + ω runs partly along the reference plane. Switching this line on shows the two side by side; the rate is always fitted on the [[park17-plane]] (ADR-0011).",
    mercury: "Totals: ≈ 575.31 ″/cy on the Park17 plane, ≈ 577.72 ″/cy on the fixed J2000 ecliptic.",
    see: ["varpi", "ecliptic-j2000", "park17-plane"],
  };
  E["heliocentric"] = {
    term: "Heliocentric", group: "frames",
    short: "Measured from the Sun's centre. Plate positions and orbital elements are body minus Sun.",
    long: "The integration itself runs about the solar-system barycentre (the [[initial-conditions]] are barycentric), and the Sun wobbles around that point, mostly because of Jupiter. For drawing and for Mercury's elements the engine subtracts the Sun's position and velocity, so the Sun sits at the origin.",
    mercury: "Mercury's perihelion is defined relative to the Sun, so heliocentric coordinates are the natural choice.",
    see: ["sun", "ecliptic-j2000"],
  };
  E["frame-caveat"] = {
    term: "Frame caveat: why not 5600 ″/cy?", group: "frames",
    short: "The famous ≈ 5600 ″/cy is measured against the moving equinox; about 5025 ″/cy of it is precession of the Earth's axis, which no inertial simulation shows.",
    typical: "observed 5599.74 ± 0.41 ″/cy (Clemence 1947, equinox of date)",
    long: "[[clemence|Clemence (1947)]] gave 5599.74 ± 0.41 ″/cy for Mercury's perihelion against the [[equinox-of-date]]. About 5025 ″/cy of that is the [[general-precession]] of the equinoxes (Clemence's value; the IAU 2006 value 5028.796195 ″/cy is marked unverified in PHY-017). An [[inertial-frame]] simulation sees only the dynamical part: ≈ 575.31 ″/cy on the [[park17-plane]] or ≈ 577.72 ″/cy on the fixed J2000 ecliptic. Simply adding 575.31 and 5028.80 does not reproduce the observed value, because the two numbers refer to different planes, so the viewer makes no comparison tighter than ±0.41 ″/cy.",
    mercury: "Of the dynamical ≈ 575 ″/cy, Newtonian planets explain ≈ 532 and GR ≈ 43: the 43 ″/cy that Le Verrier and Newcomb could not explain.",
    see: ["general-precession", "equinox-of-date", "inertial-frame", "clemence"],
  };
  E["general-precession"] = {
    term: "General precession", group: "frames",
    short: "The slow turning of the Earth's axis, which moves the equinox along the ecliptic by about 50″ per year. It is a property of the Earth, not of Mercury's orbit.",
    typical: "≈ 5025–5029 ″/cy (one turn in about 26 000 years)",
    long: "The Moon and Sun pull on the Earth's equatorial bulge and make its spin axis precess like a top. Longitudes counted from the moving equinox therefore all grow by the general precession, whatever the body. Clemence's 1947 analysis used about 5025 ″/cy; the IAU 2006 model gives p_A = 5028.796195 ″/cy (marked unverified in PHY-017).",
    mercury: "It accounts for almost 90 % of the observed ≈ 5600 ″/cy, which is why the viewer works in an [[inertial-frame]] and states the [[frame-caveat]].",
    see: ["frame-caveat", "equinox-of-date"],
  };
  E["equinox-of-date"] = {
    term: "Equinox of date", group: "frames",
    short: "The equinox at the moment of observation, which moves with the precession of the Earth's axis. Classical observations of Mercury were referred to it.",
    long: "Positions \"of date\" use the equator and equinox at the time of the observation, a slowly rotating frame. That suits telescope work but adds the [[general-precession]] to every longitude. The viewer uses the fixed [[equinox-j2000]] and [[icrf|ICRF]] axes instead.",
    mercury: "Referred to the equinox of date, Mercury's perihelion advances ≈ 5600 ″/cy; in an inertial frame, ≈ 575 ″/cy.",
    see: ["frame-caveat", "general-precession", "equinox-j2000"],
  };

  // ------------------------------------------------------------------------------------ effects
  E["planetary-perturbations"] = {
    term: "Planetary perturbations", group: "effects",
    short: "The Newtonian gravitational pull of the other planets on Mercury. Together they turn its perihelion by about 532 ″/cy.",
    typical: "≈ 532.3 ″/cy in total (Park17 Table 3)",
    long: "Each planet is integrated as a point mass with its DE440 [[gm|GM]] (planet plus moons for the outer planets and Mars; the Earth and Moon as one body, [[emb]]). Their tugs make Mercury's orbit precess. Switching a planet off removes it from the integration entirely, which is Park's \"GM set to zero\" and the [[p-remove]] protocol. Venus, Jupiter and the Earth–Moon pair dominate.",
    mercury: "This ≈ 532 ″/cy is the part Le Verrier could explain with Newton's law in 1859; the remaining ≈ 43 ″/cy needed [[gr|general relativity]].",
    see: ["venus", "jupiter", "emb", "p-remove", "preset-planets"],
  };
  const planet = (id, term, what, note, value, refValue) => {
    E[id] = {
      term: term, group: "effects",
      short: "Switches the Newtonian pull of " + what + " on and off. It turns Mercury's perihelion by about " + value + " ″/cy" + (note ? ", " + note : "") + ".",
      typical: "≈ " + value + " ″/cy (Park17 Table 3: " + refValue + ")",
      long: "When on, " + what + " is integrated as one point mass with its DE440 [[gm|GM]], starting from the DE440 [[initial-conditions]]. When off, it is removed from the run, and every remaining body starts from exactly the same state ([[p-remove]]). The table's reference value is Park et al. (2017) Table 3, measured over a 2000-year window centred on J2000, so a forward run from 1969 can differ slightly.",
      mercury: "Part of the ≈ 532 ″/cy [[planetary-perturbations|planetary]] total; switch the other planets off to see this one alone.",
      see: ["planetary-perturbations", "p-remove", "reference-value"],
    };
  };
  planet("venus", "Venus", "Venus", "the largest single planetary effect, because Venus is massive and close to Mercury", "277", "277.42");
  planet("emb", "Earth–Moon (EMB)", "the Earth–Moon barycentre (the Earth and Moon combined as one body)", "", "91", "90.89");
  planet("mars", "Mars", "the Mars system (planet and moons)", "", "2.5", "2.48");
  planet("jupiter", "Jupiter", "the Jupiter system (planet and moons)", "the second-largest effect: far away but very massive", "154", "153.99");
  planet("saturn", "Saturn", "the Saturn system (planet and moons)", "", "7.3", "7.32");
  planet("uranus", "Uranus", "the Uranus system (planet and moons)", "", "0.14", "0.14");
  planet("neptune", "Neptune", "the Neptune system (planet and moons)", "", "0.04", "0.04");
  E["emb"].long += " The Moon is not integrated separately: [[whfast|WHFast]] needs the Earth–Moon barycentre as one body, and the viewer uses it for both engines.";
  E["newtonian-gravity"] = {
    term: "Newtonian gravity", group: "effects",
    short: "Newton's inverse-square law of gravitation between point masses. Always on: the toggles add or remove bodies and corrections on top of it.",
    long: "Every run integrates the mutual Newtonian attraction of all included bodies. On its own, for just the Sun and Mercury, it gives a fixed ellipse ([[kepler-problem]]). The relativistic switches ([[gr-1pn]], [[eih]], [[lense-thirring]]) and the [[solar-j2]] term are small corrections added to it. The run's [[gauge]] reads \"Newtonian\" when none of the relativistic switches is on.",
    mercury: "Newtonian planets give ≈ 532 ″/cy of the ≈ 575 ″/cy total.",
    see: ["kepler-problem", "planetary-perturbations"],
  };
  E["gr"] = {
    term: "General relativity (GR)", group: "effects",
    short: "Einstein's theory of gravity. Its correction to the Sun's field turns Mercury's perihelion by about 43 ″/cy, the historic anomaly.",
    typical: "≈ 42.98 ″/cy (6πGM☉/(c²a(1−e²)) per orbit, about 0.10″)",
    long: "In 1859 Le Verrier found that Mercury's perihelion advanced faster than the planets could explain (about 38 ″/cy then; Newcomb later found about 43). Einstein's 1915 theory accounted for it with no adjustable parameter. In the viewer GR enters through the [[post-newtonian|1PN]] terms: [[gr-1pn]] for the Sun alone, or [[eih]] for all bodies. Its size scales with the [[ppn]] parameters as (2 + 2γ − β)/3.",
    mercury: "The per-orbit shift 6πGM☉/(c²a(1 − e²)) is about 5.02 × 10⁻⁷ rad (0.10″), times about 415 orbits per century: 42.98 ″/cy.",
    see: ["gr-1pn", "eih", "ppn", "post-newtonian", "preset-gr"],
  };
  E["post-newtonian"] = {
    term: "1PN (first post-Newtonian)", group: "effects",
    short: "The first correction to Newtonian gravity in powers of (v/c)² and GM/(rc²). It is how GR enters the equations of motion here.",
    typical: "relative size ≈ 2.5 × 10⁻⁸ at Mercury (GM☉/(rc²) and v²/c²)",
    long: "The post-Newtonian expansion writes Einstein's equations as Newton's law plus corrections ordered by small parameters such as v²/c². The first-order (1PN) terms depend on positions and velocities and produce the perihelion advance. The coordinates used to write them are a choice, the [[gauge]]; the viewer's IAS15 runs use the PPN isotropic/harmonic gauge.",
    mercury: "1PN is enough for Mercury: the 2PN correction to its advance is below 10⁻⁵ ″/cy.",
    see: ["gr-1pn", "eih", "gauge", "ppn"],
  };
  E["gr-1pn"] = {
    term: "GR 1PN (Sun)", group: "effects",
    short: "Adds the first post-Newtonian (Schwarzschild) field of the Sun only. This is the GR part of the advance.",
    typical: "≈ 42.98 ″/cy at β = γ = 1",
    long: "With [[ias15|IAS15]] the engine adds the Sun's 1PN Schwarzschild field in PPN isotropic/harmonic [[gauge]], with the [[ppn-beta|β]] and [[ppn-gamma|γ]] you set (the Sun feels no reaction). With [[whfast|WHFast]] it uses the [[gr-potential|GR effective potential]] instead, which reproduces the same secular advance but only for β = γ = 1. It is mutually exclusive with [[eih]], which contains the Sun's 1PN field.",
    mercury: "The famous ≈ 43 ″/cy: about 7.5 % of the inertial-frame total.",
    see: ["gr", "eih", "gr-potential", "ppn"],
  };
  E["eih"] = {
    term: "EIH 1PN, all bodies", group: "effects",
    short: "The Einstein–Infeld–Hoffmann equations: 1PN gravity among all bodies, not just around the Sun. Advanced; IAS15 only; replaces GR 1PN.",
    typical: "≈ 42.98 ″/cy; IAS15 about 1.3× slower",
    long: "Einstein, Infeld and Hoffmann (1938) derived the 1PN equations of motion for N bodies. They contain the Sun's Schwarzschild field ([[gr-1pn]]) plus the planets' own small 1PN terms and cross terms, which is what JPL's ephemerides use. Because they depend on velocities, only [[ias15|IAS15]] can integrate them. Switching EIH on switches GR 1PN off.",
    mercury: "The planets' extra 1PN terms change Mercury's rate only slightly; the reference shown is the Sun-only 42.98 ″/cy.",
    see: ["gr-1pn", "post-newtonian", "ias15"],
  };
  E["ppn"] = {
    term: "PPN parameters", group: "effects",
    short: "The parametrized post-Newtonian formalism describes many theories of gravity with a few numbers; GR has β = γ = 1. The GR advance scales as (2 + 2γ − β)/3.",
    long: "PPN theory writes the 1PN metric with coefficients that differ between theories. The viewer exposes two: [[ppn-beta|β]] and [[ppn-gamma|γ]]. They are used by [[gr-1pn]] (IAS15), [[eih]], the Rung-lab GR rung and, through γ, [[lense-thirring]]. [[whfast|WHFast]] can only do β = γ = 1. Solar-system experiments find both equal to 1 to within about 10⁻⁴ or better.",
    mercury: "Mercury's perihelion is one of the classic measurements of 2γ − β: change β or γ and watch the GR contribution scale.",
    see: ["ppn-beta", "ppn-gamma", "gr"],
  };
  E["ppn-beta"] = {
    term: "PPN β", sym: "β", group: "effects",
    short: "How non-linear gravity is: how much the field of a mass is changed by gravity itself. GR: β = 1.",
    typical: "1 (GR); allowed |β − 1| ≤ 100",
    long: "β multiplies the term in the metric that is quadratic in the Newtonian potential. Raising β reduces the perihelion advance, since the GR rate scales as (2 + 2γ − β)/3. It is used by [[gr-1pn]] (IAS15 only unless β = γ = 1), [[eih]] and the Rung-lab GR rung; the field is disabled when none of those is on.",
    mercury: "β = 2 with γ = 1 gives (2 + 2 − 2)/3 = 2/3 of the GR advance, about 28.65 ″/cy.",
    see: ["ppn", "ppn-gamma"],
  };
  E["ppn-gamma"] = {
    term: "PPN γ", sym: "γ", group: "effects",
    short: "How much space curvature a unit mass produces. GR: γ = 1.",
    typical: "1 (GR); allowed |γ − 1| ≤ 100",
    long: "γ also governs light bending and the Shapiro delay, which measure it very precisely (Cassini: γ − 1 of order 10⁻⁵). The GR perihelion advance scales as (2 + 2γ − β)/3, and the [[lense-thirring]] term depends on γ too. Like β it applies to [[gr-1pn]], [[eih]], the Rung-lab GR rung and Lense–Thirring, and needs IAS15 unless it is 1.",
    mercury: "γ = 0 with β = 1 gives (2 + 0 − 1)/3 = 1/3 of the GR advance, about 14.33 ″/cy.",
    see: ["ppn", "ppn-beta", "lense-thirring"],
  };
  E["gr-potential"] = {
    term: "GR effective potential", group: "effects",
    short: "WHFast's way of including GR: an extra term in the Sun's potential that reproduces the 1PN perihelion advance. Only for β = γ = 1.",
    long: "The symplectic [[whfast|WHFast]] map cannot take the velocity-dependent 1PN force directly, so the engine uses the Nobili–Roxburgh effective potential instead. It gives the correct secular perihelion advance but not the 1PN mean motion, so the orbital phase differs slightly from an IAS15 1PN run. The run's [[gauge]] reads \"GR effective potential (Nobili–Roxburgh)\".",
    mercury: "WP29 measured 42.9796 ″/cy with it against 42.98 for the true 1PN field.",
    see: ["gr-1pn", "whfast", "gauge"],
  };
  E["solar-j2"] = {
    term: "Solar J2 (oblateness)", sym: "J₂", group: "effects",
    short: "The Sun is very slightly flattened by its rotation; the extra pull of its equatorial bulge turns Mercury's perihelion by about 0.03 ″/cy.",
    typical: "J₂ ≈ 2.2 × 10⁻⁷ (DE440); ≈ 0.028 ″/cy",
    long: "J₂ is the quadrupole coefficient of the Sun's gravity field. The engine uses DE440's J2SUN (read from the embedded [[header-440]]) and solar radius, about the WGCCRE solar pole, with the reaction on the Sun. The planar estimate 3πJ₂R☉²/(a²(1 − e²)²) per orbit gives 0.0279 ″/cy; with the real pole geometry FerroAstro measures 0.02845 ″/cy. The [[j2-override]] scales it for demonstration.",
    mercury: "Small, but it once mattered: a large solar J₂ was proposed as an alternative to GR, and helioseismology rules that out.",
    see: ["j2-override", "physical-exaggeration", "three-sigma"],
  };
  E["lense-thirring"] = {
    term: "Lense–Thirring", group: "effects",
    short: "Frame dragging by the Sun's rotation, a gravitomagnetic GR effect. It changes Mercury's perihelion by about −0.002 ″/cy. IAS15 only.",
    typical: "≈ −0.002 ″/cy (Park17; FerroAstro −0.002015)",
    long: "A rotating mass drags inertial frames around with it (Lense and Thirring, 1918). The engine uses the DE440 formulation, with the Sun's spin angular momentum G·S = GM☉ · MOISUN · ASUN² · OMGSUN from DE440 constants and γ from the [[ppn]] parameters. Because the force depends on velocity, only [[ias15|IAS15]] can integrate it. The [[spin-scale]] multiplies G·S for demonstration.",
    mercury: "Far below what the table resolves on typical spans: at 300–400 years each leg's jack-knife σ is about 0.01–0.02 ″/cy, so 3σ is roughly 0.05–0.08 ″/cy, some 25 to 40 times the 0.002 ″/cy effect, and the row stays greyed. Treat it as a check of sign and order of magnitude.",
    see: ["spin-scale", "gr", "three-sigma"],
  };
  E["asteroids"] = {
    term: "Big asteroids", group: "effects",
    short: "Adds the first N numbered asteroids in DE440's catalogue order (header.440) as point masses. Their effect on Mercury is about 0.001 ″/cy. IAS15 only.",
    typical: "N = 16 by default (1 … 343); ≈ 0.001 ″/cy (Park17: 0.0012 for all 343)",
    long: "DE440 models 343 asteroids individually. The viewer takes the first [[asteroid-count|N]] of them, with states at the run epoch from the in-repository REF-003 file (provisional), as light bodies that are pulled by the Sun and planets and pull back on them. They require [[ias15|IAS15]]: WHFast integrates only major point masses.",
    mercury: "Far below what the table resolves on typical spans (3σ is several hundredths of ″/cy at 300–400 years, against about 0.001 ″/cy). DESIGN.md §8 E8 asks only for the right sign and order of magnitude (0 … 0.002 ″/cy).",
    see: ["asteroid-count", "three-sigma"],
  };
  E["gauge"] = {
    term: "Gauge (coordinates)", group: "effects",
    short: "The coordinate convention in which post-Newtonian equations are written. Different gauges give different-looking equations but the same observable results.",
    long: "At 1PN order, \"where\" a body is depends on the coordinate choice: isotropic/harmonic, Schwarzschild areal, and so on. Measurable quantities such as the secular perihelion advance do not depend on it, but intermediate quantities can. The metadata line names the run's gauge: \"PPN isotropic/harmonic\" for IAS15 GR, EIH or Lense–Thirring; \"GR effective potential (Nobili–Roxburgh)\" for WHFast GR; \"Newtonian\" otherwise.",
    mercury: "The Rung lab shows that splitting the 43 ″/cy into \"special-relativistic\" and \"GR\" parts depends on the gauge ([[sr-gr-split]]).",
    see: ["post-newtonian", "sr-gr-split", "metadata"],
  };
  E["rung-lab"] = {
    term: "Rung lab", group: "effects",
    short: "Mercury as a test particle around a fixed Sun, to compare central-force models (Newton, special-relativity variants, GR) one at a time.",
    typical: "epoch J2000.0; IAS15 (test particle); no planets",
    long: "The Rung lab reproduces FerroAstro's single-body \"ladder\": Mercury starts from the Standish & Williams (1992) J2000 mean elements ([[sw92]]) on the J2000 mean ecliptic, with μ = GM☉, and the [[rung]] selector picks the central force. Only the [[solar-j2]] and [[lense-thirring]] toggles apply; planets, asteroids and EIH are off. The contribution table adds a row for the relativistic central term (the leg with the rung set to Newton).",
    mercury: "It shows how much of the 43 ″/cy a merely \"relativistic\" label buys, and that the SR/GR split is not observable.",
    see: ["rung", "test-particle", "sr-a", "sr-b", "system-mode"],
  };
  E["rung"] = {
    term: "Rung (central-force model)", group: "effects",
    short: "Chooses the Sun's field in the Rung lab: Newton (0), SR-a (≈ 7.16 ″/cy), SR-b harmonic (≈ 50.14), SR-b areal (≈ 35.82) or GR with β, γ (≈ 42.98).",
    long: "The rungs are the ADR-0012 definitions. Newton only gives no advance ([[kepler-problem]]). [[sr-a]] adds special-relativistic kinematics to Newton's potential. [[sr-b]] keeps only the velocity-dependent 1PN terms, in one of two [[gauge|gauges]]. The GR rung is the Sun's 1PN Schwarzschild field with your [[ppn]] β and γ.",
    mercury: "The SR values are GR/6, 7/6 GR and 5/6 GR: different \"relativistic\" models give very different answers, and only GR matches observation.",
    see: ["rung-lab", "sr-a", "sr-b", "sr-gr-split"],
  };
  E["sr-a"] = {
    term: "SR-a (SrKepler)", group: "effects",
    short: "Special-relativistic kinematics in Newton's potential, H = γmc² − μm/r, with no post-Newtonian gauge. Gives about 7.16 ″/cy, one sixth of GR.",
    typical: "7.1634 ″/cy",
    long: "SR-a treats Mercury as a relativistic particle (Lorentz factor γ) moving in the plain Newtonian potential of a fixed Sun. It is a [[test-particle]] definition from ADR-0012 and has no meaning with the planets on. To leading (first post-Newtonian) order the result is one sixth of the GR advance, a classic textbook result.",
    mercury: "Shows that special relativity alone explains only about 7 of the 43 ″/cy.",
    see: ["rung", "sr-b", "sr-gr-split"],
  };
  E["sr-b"] = {
    term: "SR-b (velocity-only 1PN terms)", group: "effects",
    short: "Keeps only the velocity-dependent 1PN terms of the GR equations. The result depends on the gauge: ≈ 50.14 ″/cy (harmonic) or ≈ 35.82 ″/cy (areal, pedagogical).",
    typical: "harmonic 50.144 ″/cy (7/6 GR); areal 35.817 ″/cy (5/6 GR)",
    long: "SR-b splits the 1PN Lagrangian into velocity terms (\"special-relativistic\") and potential terms, and keeps only the former. In the PPN isotropic/harmonic [[gauge]] this gives 7/6 of GR; in Schwarzschild areal coordinates 5/6. The two answers differ because the split itself depends on the coordinates.",
    mercury: "A demonstration that \"how much of the advance is special relativity\" has no unique answer ([[sr-gr-split]]).",
    see: ["rung", "sr-a", "gauge", "sr-gr-split"],
  };
  E["sr-gr-split"] = {
    term: "SR/GR split (not observable)", group: "effects",
    short: "Dividing the relativistic advance into \"special\" and \"general\" relativistic parts depends on the coordinates chosen, so it is not a measurable quantity.",
    long: "Only the total 1PN advance (≈ 42.98 ″/cy) is coordinate-independent. The SR-b rungs give 50.14 or 35.82 ″/cy depending on the [[gauge]], so neither is \"the SR part\". ADR-0012 records this, and the viewer offers the SR rungs only in the [[rung-lab]] with their gauge labels.",
    mercury: "Quote 42.98 ″/cy as the GR advance; avoid splitting it.",
    see: ["sr-a", "sr-b", "gauge"],
  };
  E["j2-override"] = {
    term: "Solar J2 override", group: "effects",
    short: "Replaces the Sun's DE440 J₂ with your value. Empty means DE440. Any value marks the run non-nominal (exaggerated physics).",
    typical: "empty = DE440 J2SUN (≈ 2.2 × 10⁻⁷); range 0 … 1 × 10⁻³",
    long: "The J₂ contribution scales linearly with J₂, so a value 1000 times DE440 makes the 0.03 ″/cy effect about 28 ″/cy and easy to see in a short run. This changes the physics, unlike the [[display-exaggeration]], so the header shows \"Exaggerated physics\" and the metadata says the run is not [[nominal-run|nominal]]. The table's J2 reference value is scaled by J₂/J₂(DE440). Only active when the Solar J2 toggle is on.",
    mercury: "Useful for seeing what the J₂ term does; real-Sun numbers need the field empty.",
    see: ["solar-j2", "physical-exaggeration", "nominal-run"],
  };
  E["spin-scale"] = {
    term: "Solar spin scale", group: "effects",
    short: "Multiplies the Sun's DE440 spin angular momentum G·S used by Lense–Thirring. 1 is the real Sun; anything else marks the run non-nominal.",
    typical: "1 (DE440); range 0 … 1 000 000",
    long: "The Lense–Thirring advance is proportional to G·S, so a scale of 10⁴ turns −0.002 ″/cy into about −20 ″/cy. Like the [[j2-override]] it changes the physics and is labelled \"Exaggerated physics\"; the table's reference value is scaled by the same factor. Only active when the [[lense-thirring]] toggle is on (IAS15).",
    mercury: "A way to watch frame dragging act on the perihelion within a short run.",
    see: ["lense-thirring", "physical-exaggeration"],
  };
  E["physical-exaggeration"] = {
    term: "Physical exaggeration (non-nominal run)", group: "effects",
    short: "Settings that change the actual physics or accuracy: a J₂ override, a solar spin scale other than 1, an IAS15 ε looser than 10⁻⁹, or conformal-gravity coefficients other than the fit values. Such runs are labelled \"Exaggerated physics\".",
    long: "These knobs exist to make tiny effects visible, but the numbers they produce are not the real solar system. The page flags the run in the header badge, in the readout (\"non-nominal\") and in the metadata (\"nominal\": false). Compare the [[display-exaggeration]], which changes only the drawing.",
    mercury: "Use them to build intuition, never for literature comparisons.",
    see: ["j2-override", "spin-scale", "ias15-epsilon", "nominal-run", "display-exaggeration", "cg-fit-values"],
  };

  // ------------------------------------------------------------------------- comparison models
  // Sources: ADR-0016, PHY-020, OI-79 and the rd#88 research notes (REPORT.md, the physics note).
  // The Mercury numbers marked "Gauss" are the first-order averages the engine evaluates
  // (metadata "conformal.reference"); they reproduce the rd#88 values.
  E["comparison-model"] = {
    term: "Comparison model", group: "comparison",
    short: "A beyond-GR term the viewer can switch on to compare with GR. It is off by default and never part of the GR ladder, the 575.31 ″/cy total or any reference value.",
    long: "FerroAstro's ladder reproduces the observed advance from Newtonian planets, solar J2 and GR only. A comparison model (ADR-0016) is a different theory's term, added on request so its prediction can be set against GR and against ephemeris bounds such as the [[inpop15a-bound]]. Wherever one appears (toggle, preset, table row, gauge, scenario, metadata) it says \"comparison model\". Its table reference is the engine's own [[cg-gauss-reference|Gauss estimate]], not a literature or ladder value. The first one is [[conformal-gravity]] (toggle bit 12, viewer ABI v2).",
    mercury: "With the default uniform [[gamma0-centring]] it adds about nothing; Sun-centred, its γ0 term is about −0.18 ″/cy, some 60 times the INPOP15a bound.",
    see: ["conformal-gravity", "cg-gauss-reference", "inpop15a-bound", "cg-fit-values"],
  };
  E["conformal-gravity"] = {
    term: "Conformal gravity (comparison model)", sym: "CG", group: "comparison",
    short: "Mannheim's fourth-order theory from the Weyl action. Near the Sun it adds a constant pull γc²/2 and a Hooke-like push κc²r to Newton. Toggle bit 12, off by default; a comparison model, not part of the GR ladder.",
    typical: "Gauss, fit values: γ* term −3.2×10⁻¹² ″/cy, κ term +9.9×10⁻¹² ″/cy; γ0 Sun-centred −0.183 ″/cy, uniform ≈ 0",
    long: "The [[weyl-action]] has the [[mannheim-kazanas-metric]] as its static exterior, B = 1 − 2β/r + γr − κr². With γ = κ = 0 it is Schwarzschild, so its β term gives GR's +42.98 ″/cy, recovered rather than predicted (β is matched to the Keplerian GM); this viewer takes that part from its own GR switches. The extra terms are, to Newtonian order, an inward acceleration (γc²/2) r̂ with γ = γ* + γ0 and an outward κc² r. The engine adds them as one velocity-independent, Sun-sourced term with reaction on the Sun, gauge \"static weak-field potential\", so it combines with GR 1PN, EIH and Lense–Thirring. Parameters: [[gamma0]], [[gamma-star]], [[kappa]] and the [[gamma0-centring]].",
    mercury: "Only γ0 could matter. Uniform, it moves the Sun and planets alike and changes nothing; Sun-centred, it gives about −0.183 ″/cy, excluded by the [[inpop15a-bound]]. The γ* and κ terms are about 10⁻¹¹ ″/cy, far below what any run resolves.",
    see: ["comparison-model", "weyl-action", "mannheim-kazanas-metric", "gamma0-centring", "cg-gauss-reference"],
  };
  E["weyl-action"] = {
    term: "Weyl action", group: "comparison",
    short: "The action of conformal gravity: the square of the Weyl curvature tensor, which is unchanged when the metric is rescaled by any positive function, g → Ω²g.",
    long: "Mannheim's theory replaces the Einstein–Hilbert action by I_W = −α_g ∫ d⁴x √−g C_λμνκ C^λμνκ, with C the Weyl (conformal) tensor. Local conformal invariance fixes this form and makes the field equations fourth order, so the static vacuum solution has more integration constants than Schwarzschild's one: the β, γ and κ of the [[mannheim-kazanas-metric]]. A conformally flat metric (Ω²η) has zero Weyl tensor and is invisible to this action.",
    mercury: "It is the origin of the extra γ and κ terms that the [[conformal-gravity]] toggle adds; the action itself is not evaluated by the engine.",
    see: ["conformal-gravity", "mannheim-kazanas-metric"],
  };
  E["mannheim-kazanas-metric"] = {
    term: "Mannheim–Kazanas metric", group: "comparison",
    short: "The static, spherically symmetric vacuum solution of conformal gravity: ds² = B dt² − dr²/B − r²dΩ² with B = 1 − 2β/r + γr − κr² in the form used for orbits.",
    typical: "β = GM/c² (Sun: about 1.48 km)",
    long: "Mannheim and Kazanas (1989, ApJ 342, 635) found the exact form B = 1 − β(2 − 3βγ)/r − 3βγ + γr − κr². The orbit form drops the βγ terms, which are below 10⁻²⁴ in the solar system. To Newtonian order Φ = c²(B − 1)/2, so the γr term is a constant inward acceleration γc²/2 and the −κr² term an outward acceleration κc²r. The −dr²/B part is what gives PPN γ = 1 and GR's light bending.",
    mercury: "The 1/r term alone is Schwarzschild and gives the +42.98 ″/cy; the viewer's [[conformal-gravity]] toggle adds only the γ and κ accelerations.",
    see: ["conformal-gravity", "weyl-action", "gamma0", "kappa"],
  };
  E["gamma0"] = {
    term: "γ0, cosmological linear coefficient", sym: "γ0", group: "comparison",
    short: "The linear-potential coefficient attributed to the rest of the universe. Its galactic-fit value is unverified (OI-79). Where it is centred decides whether it acts on Mercury at all.",
    typical: "fit 3.06×10⁻³⁰ cm⁻¹ (default, unverified); input range ±10⁻²⁵ cm⁻¹, signed",
    long: "Rotation-curve fits write the linear coefficient as γ = N*γ* + γ0, with N* the source mass in solar masses ([[gamma-star]]). Mannheim derives γ0 from a negatively curved Robertson–Walker cosmology seen by an observer at rest in a galaxy, and the fits measure γ0c²R/2 from each galaxy's centre. The sources leave open where it is centred in the solar system ([[gamma0-centring]]). γ > 0 attracts and makes the perihelion regress. Any value other than the fit value marks the run as [[physical-exaggeration|exaggerated physics]].",
    mercury: "Sun-centred at the fit value: Gauss −0.183 ″/cy, about 60× the [[inpop15a-bound]]; uniform: 0. Sultana, Kazanas and Said (2012) bound a Sun-centred γ below 1.5×10⁻³¹ cm⁻¹.",
    see: ["gamma0-centring", "gamma-star", "cg-fit-values", "inpop15a-bound"],
  };
  E["gamma-star"] = {
    term: "γ*, linear coefficient per solar mass", sym: "γ*", group: "comparison",
    short: "The linear-potential coefficient each solar mass of a source carries. For the Sun (N* = 1) it is always Sun-centred. Fit value unverified (OI-79).",
    typical: "fit 5.42×10⁻⁴¹ cm⁻¹ per M☉ (default, unverified); input range ±10⁻²⁵ cm⁻¹, signed",
    long: "In γ = N*γ* + γ0 the γ* part belongs to the source itself, so for the Sun it acts like a Sun-centred γ whatever the [[gamma0-centring]]. The engine adds it to the Sun-sourced term with reaction on the Sun. The fit value comes from galactic rotation curves; it is about 10⁻¹¹ of the fitted [[gamma0]].",
    mercury: "Gauss at the fit value: about −3.2×10⁻¹² ″/cy, some nine orders of magnitude below what a run resolves. Set it much larger to see the retrograde effect; the run is then labelled exaggerated.",
    see: ["gamma0", "conformal-gravity", "cg-fit-values"],
  };
  E["kappa"] = {
    term: "κ, quadratic coefficient", sym: "κ", group: "comparison",
    short: "The coefficient of the −κr² term of the Mannheim–Kazanas B. κ > 0 is a repulsive, Hooke-like acceleration κc²r that makes the perihelion advance (prograde). Signed; fit value unverified (OI-79).",
    typical: "fit 9.54×10⁻⁵⁴ cm⁻² (default, unverified); input range ±10⁻⁴⁰ cm⁻², signed",
    long: "Because B contains −κr², the Newtonian-order potential c²(B − 1)/2 has −κc²r²/2 and the acceleration is +κc² r, outward for κ > 0. (An earlier integration note assumed an attractive κ; the rd#88 physics note and ADR-0016 supersede it.) It has the r-dependence of a cosmological-constant term. Mannheim and O'Brien (2012) fit it to galactic rotation curves.",
    mercury: "Gauss at the fit value: about +9.9×10⁻¹² ″/cy, far below resolution. The rate grows as a³, so it matters only on galactic scales.",
    see: ["conformal-gravity", "mannheim-kazanas-metric", "cg-fit-values"],
  };
  E["gamma0-centring"] = {
    term: "γ0 centring (uniform or Sun-centred)", group: "comparison",
    short: "Uniform (default): γ0 gives every body the same acceleration γ0c²/2, so heliocentric orbits are unchanged. Sun-centred: γ0 adds to the Sun's own γ and pulls Mercury towards the Sun.",
    typical: "uniform: 0; Sun-centred at the fit γ0: Gauss −0.183 ″/cy",
    long: "The sources do not settle where γ0 acts (ADR-0016 decision D2), so both readings are computed, not just labelled. Uniform: the same acceleration on every body along a fixed direction; relative motion is unchanged, so with velocity-independent forces the heliocentric rate is unchanged exactly (to round-off). With EIH or Lense–Thirring on, whose forces depend on velocity, the common drift (about 0.04 m/s per century at the fit γ0) enters at O(v·Δv/c²): not exactly zero, but far below any resolvable rate. In the Rung lab the fixed Sun stands for the freely falling heliocentric frame, so a uniform γ0 adds no term there. Sun-centred: γ = γ* + γ0 in the Sun-sourced term.",
    mercury: "This choice is the whole question at Mercury: uniform gives 0, Sun-centred about −0.183 ″/cy against the [[inpop15a-bound]] of ±0.0031 ″/cy.",
    see: ["gamma0", "inpop15a-bound", "conformal-gravity", "eih", "lense-thirring"],
  };
  E["inpop15a-bound"] = {
    term: "INPOP15a bound", group: "comparison",
    short: "The INPOP15a planetary ephemeris finds no supplementary advance of Mercury's perihelion: 0.0 ± 3.1 mas/cy (0.0 ± 0.0031 ″/cy).",
    typical: "0.0 ± 3.1 mas/cy (stricter criterion: ± 1.05 mas/cy); EPM2011: −2 ± 3 mas/cy",
    long: "Fienga, Laskar, Manche and Gastineau (2016, arXiv:1601.00947), with MESSENGER ranging, fit a supplementary perihelion advance on top of GR and find it consistent with zero. Any extra term must therefore give |Δϖ̇| within a few mas/cy at Mercury; for a Sun-centred linear potential that means |γ| ≲ 5×10⁻³² cm⁻¹. The proceedings volume and pages have not been checked (ADR-0016).",
    mercury: "A Sun-centred γ0 at the fit value (−183 mas/cy) misses it by a factor of about 60; a uniform γ0 passes trivially.",
    see: ["gamma0-centring", "gamma0", "comparison-model"],
  };
  E["cg-gauss-reference"] = {
    term: "Gauss reference (conformal gravity)", group: "comparison",
    short: "The engine's analytic rate for the current γ0, γ*, κ and centring: first-order Gauss secular averages for Mercury, labelled comparison model. The page computes nothing itself.",
    long: "Per orbit the linear term gives −πγa²√(1−e²)/β and the quadratic term +3πκa³√(1−e²)/β, with β = GM☉/c² (DE440), a and e Mercury's SW92 J2000 mean elements, and the rate from the two-body mean motion. A uniform γ0 contributes exactly 0. The factor is √(1−e²), checked by numerical integration in the rd#88 notes; the (1−e²)² printed by Sultana et al. (2012) is about 7 % off for Mercury. It is the table's reference for the CG row and appears in the metadata (\"conformal.reference\").",
    mercury: "At the fit values: γ* −3.2×10⁻¹², κ +9.9×10⁻¹² ″/cy; a Sun-centred fit γ0 adds −0.183 ″/cy. The engine's tests check it against integrations with exaggerated parameters.",
    see: ["conformal-gravity", "reference-value", "comparison-model"],
  };
  E["cg-fit-values"] = {
    term: "Conformal-gravity fit values (unverified)", group: "comparison",
    short: "γ0 = 3.06×10⁻³⁰ cm⁻¹, γ* = 5.42×10⁻⁴¹ cm⁻¹ per M☉, κ = 9.54×10⁻⁵⁴ cm⁻²: galactic rotation-curve fits, the viewer's defaults, UNVERIFIED (open item OI-79).",
    long: "They are quoted from a secondary transcription of Mannheim's lecture notes; the abstract of Mannheim and O'Brien (2012, PRD 85, 124020) confirms γ0 and κ but the full paper has not been checked (OI-79). They are display defaults only and never test oracles: the engine's tests use exaggerated parameters against the analytic Gauss rates. Changing any of them marks the run as [[physical-exaggeration|exaggerated physics]]; \"Reset to fit values\" restores them.",
    mercury: "At these values only the γ0 centring matters (−0.183 ″/cy Sun-centred, 0 uniform).",
    see: ["gamma0", "gamma-star", "kappa", "physical-exaggeration"],
  };

  // -------------------------------------------------------------------------------- integrators
  E["integrator"] = {
    term: "Integrator (engine)", group: "integrators",
    short: "The numerical method that steps the equations of motion forward in time. This viewer offers WHFast and IAS15.",
    long: "An integrator turns forces into trajectories by taking many small time steps. Methods differ in speed, accuracy and which forces they can handle. [[whfast|WHFast]] is fast and [[symplectic]] but limited to forces it can split; [[ias15|IAS15]] is adaptive, very accurate and handles velocity-dependent forces. The resolved engine is shown in the header badge, next to the selector and in the transport bar.",
    mercury: "A good integrator gives ϖ̇ ≈ 0 for the [[kepler-problem]]; the table's [[integrator-floor]] shows how close.",
    see: ["engine-auto", "whfast", "ias15", "integrator-floor"],
  };
  E["engine-auto"] = {
    term: "Engine: AUTO", group: "integrators",
    short: "Lets the engine choose: WHFast when it can express every switched-on effect, otherwise IAS15. The resolved choice is shown next to the selector.",
    long: "AUTO tries the [[whfast|WHFast]] scenario first and falls back to [[ias15|IAS15]] when the propagator refuses it ([[engine-refusal|FA-E1201]]). WHFast is refused for [[lense-thirring]], [[asteroids]], [[eih]], and GR with β or γ other than 1. Forcing WHFast with one of those is refused with the engine's message. The contribution table always uses one engine for every leg.",
    mercury: "The default full model includes Lense–Thirring, so AUTO resolves to IAS15 there; \"Planets only\" or \"GR only\" resolve to WHFast.",
    see: ["whfast", "ias15", "engine-refusal"],
  };
  E["whfast"] = {
    term: "WHFast", group: "integrators",
    short: "A fast symplectic Wisdom–Holman integrator: each step is an exact Kepler orbit plus a kick from the other bodies. Fixed step, no long-term energy drift.",
    typical: "step 1 day (≈ 88 steps per Mercury orbit), corrector order 11",
    long: "The Wisdom–Holman method (1991; WHFast implementation by Rein and Tamayo, 2015) splits the motion into Keplerian drifts around the Sun and interaction kicks, in Jacobi coordinates. Symplectic correctors of order 11 remove most of the step-size error. Being [[symplectic]], it shows no secular drift in ϖ from the integrator, only a small step-dependent phase error. It cannot take velocity-dependent forces, so GR enters as the [[gr-potential]] and LT, asteroids and EIH need IAS15.",
    mercury: "Fast enough to run centuries in seconds; WP29 measured the GR rung at 42.9796 ″/cy with it.",
    see: ["symplectic", "wh-step", "gr-potential", "ias15"],
  };
  E["ias15"] = {
    term: "IAS15", group: "integrators",
    short: "A 15th-order adaptive Gauss–Radau integrator that chooses its own step sizes and is accurate to near machine precision. Handles every effect in the viewer.",
    typical: "ε = 10⁻⁹ (nominal); slower than WHFast",
    long: "IAS15 (Rein and Spiegel, 2015) adapts the step so that its error estimate stays below the precision parameter [[ias15-epsilon|ε]]; at the nominal ε its error grows only through round-off (Brouwer's law). It can integrate velocity-dependent forces, so it is required for [[lense-thirring]], [[asteroids]], [[eih]] and GR with general [[ppn]] β, γ. In the [[rung-lab]] it runs as \"IAS15 (test particle)\". Mercury's states are sampled every day for the passage search.",
    mercury: "The reference engine for the full model's ≈ 575.31 ″/cy.",
    see: ["ias15-epsilon", "whfast", "engine-auto"],
  };
  E["symplectic"] = {
    term: "Symplectic integrator", group: "integrators",
    short: "An integrator that preserves the geometric structure of Hamiltonian motion, so energy errors oscillate instead of growing. Ideal for long orbital runs.",
    long: "A symplectic map exactly conserves a slightly perturbed Hamiltonian. In practice the energy error stays bounded over very long times (NUM-004: ≤ 10⁻⁸ over a million years for WHFast) and there is no artificial secular drift in orbital elements such as ϖ. Non-symplectic methods such as [[ias15|IAS15]] instead keep errors at round-off level by taking very accurate steps.",
    mercury: "No integrator-made perihelion drift: a step-size error shows up as a small phase error, not as a fake advance.",
    see: ["whfast", "ias15"],
  };
  E["wh-step"] = {
    term: "WHFast step", group: "integrators",
    short: "WHFast's fixed time step, in days. Smaller is more accurate and slower; it also sets how often Mercury is sampled for the passage search.",
    typical: "1 day (default); range 0.125 … 4 days",
    long: "At 1 day WHFast takes about 88 steps per Mercury orbit. The [[display-cadence]] must be at least the step. Mercury's state is sampled every step to locate [[perihelion-passage|perihelion passages]], so a coarse step also coarsens the passage interpolation. Not used by IAS15 or the Rung lab.",
    mercury: "At the default 1-day step the GR-potential rung gives 42.9796 ″/cy (WP29), against 42.9805 ″/cy analytic.",
    see: ["whfast", "display-cadence"],
  };
  E["ias15-epsilon"] = {
    term: "IAS15 ε (precision)", sym: "ε", group: "integrators",
    short: "IAS15's accuracy parameter: smaller ε means smaller steps and higher accuracy. Values looser than 10⁻⁹ are marked \"fast, reduced accuracy\" and non-nominal.",
    typical: "10⁻⁹ (nominal); range 10⁻¹² … 10⁻⁶",
    long: "IAS15 chooses each step so that its estimate of the truncation error, relative to the acceleration, stays below ε. The FerroAstro ladder and PHY-012 runs use 10⁻⁹. Larger values run faster but are no longer round-off-limited, so the run is flagged as [[physical-exaggeration|non-nominal]].",
    mercury: "Loosening ε can add a spurious drift in ϖ; check the [[integrator-floor]] if you do.",
    see: ["ias15", "physical-exaggeration", "nominal-run"],
  };
  E["integrator-floor"] = {
    term: "Integrator floor (Kepler-only rate)", group: "table",
    short: "The rate measured for Sun + Mercury alone with Newtonian gravity, where the true answer is exactly 0. What it shows is integrator and estimator error.",
    typical: "|ϖ̇| ≤ 0.001 ″/cy over 100 years",
    long: "Every contribution table runs a Kepler-only leg with the same engine, span and estimator as the others (in the Rung lab: the Newton rung). Its rate should be zero ([[kepler-problem]]); any value is numerical error in the integration, the passage interpolation and the fit. The per-effect wedges on the chart stack on top of this floor.",
    mercury: "If the floor is much smaller than a contribution, that contribution is not an artefact.",
    see: ["kepler-problem", "contribution-table", "cross-terms"],
  };
  E["engine-refusal"] = {
    term: "Engine refusal (FA-E1201)", group: "integrators",
    short: "The message when the chosen engine cannot model a switched-on effect, e.g. WHFast with Lense–Thirring. Choose IAS15 or AUTO.",
    long: "The propagator decides what each integrator can express; the viewer never keeps its own copy of the rules. [[whfast|WHFast]] refuses velocity-dependent or minor-body terms ([[lense-thirring]], [[asteroids]], [[eih]]) and GR with β or γ ≠ 1. With [[engine-auto|AUTO]] the refusal just selects IAS15; with WHFast forced, the error banner shows the FA-E1201 text, which names the switch.",
    mercury: "No effect on the physics: it only tells you which engine to use.",
    see: ["engine-auto", "whfast", "error-codes"],
  };
  E["wasm-engine"] = {
    term: "FerroAstro engine (wasm)", group: "integrators",
    short: "The physics engine, compiled from Rust to WebAssembly and run in background workers. The page only draws what it reports.",
    long: "The same Rust crates that power FerroAstro's command-line tools run inside the browser as WebAssembly, with no network access and no imports. One worker drives the live run; the contribution table runs its legs in further workers in parallel. JavaScript computes no orbits, forces or fits: only drawing geometry, calendar labels and the table's differences of rates.",
    mercury: "Every number on the page can be reproduced with the CLI from the [[scenario-toml]].",
    see: ["scenario-toml", "metadata"],
  };

  // ---------------------------------------------------------------------------------------- time
  E["arcsec"] = {
    term: "Arcsecond", sym: "″", group: "time",
    short: "1/3600 of a degree. The chart's y axis and Δϖ are in arcseconds.",
    typical: "1″ ≈ 4.85 × 10⁻⁶ rad; 1° = 3600″",
    long: "An arcsecond is the angle a coin 2 cm across subtends at about 4 km. Orbital longitudes in the engine are radians; the page shows degrees for orbital angles and arcseconds for small changes such as the perihelion advance. The symbol ″ follows a number, as in 43″.",
    mercury: "Mercury's perihelion turns about 1.4″ per orbit: roughly the size of a coin seen from 3 km.",
    see: ["arcsec-per-century", "varpi"],
  };
  E["arcsec-per-century"] = {
    term: "Arcseconds per century", sym: "″/cy", group: "time",
    short: "The unit of every rate on the page: arcseconds per Julian century (36525 days).",
    typical: "full model ≈ 575 ″/cy ≈ 0.16° per century",
    long: "Perihelion rates are traditionally quoted in arcseconds per [[julian-century]]. To convert to arcseconds per orbit, divide by about 415.2 (Mercury's orbits per century); to degrees per thousand years, divide by 360.",
    mercury: "GR's 42.98 ″/cy is about 0.1035″ per orbit.",
    see: ["arcsec", "julian-century", "varpi-dot"],
  };
  E["julian-year"] = {
    term: "Julian year", group: "time",
    short: "Exactly 365.25 days of 86 400 SI seconds. All 'years' on the page (span, chart x axis, speed) are Julian years.",
    long: "Astronomy uses the Julian year as a fixed unit, independent of the calendar. The chart's lower axis counts Julian years since the [[run-epoch]]; the upper axis labels them as epoch years (2000.0 plus Julian years from J2000.0), close to calendar years.",
    mercury: "Mercury makes about 4.15 orbits per Julian year.",
    see: ["julian-century", "run-epoch"],
  };
  E["julian-century"] = {
    term: "Julian century", group: "time",
    short: "100 Julian years: exactly 36 525 days. The 'cy' in ″/cy.",
    long: "The fit's time variable t is measured in Julian centuries from [[j2000|J2000.0]] TDB, so the fitted [[fit-b|B]] is the longitude at J2000.0 and [[varpi-dot|ϖ̇]] the slope there. Rates per century are the traditional unit for secular motions.",
    mercury: "Mercury completes about 415.2 orbits per Julian century.",
    see: ["julian-year", "arcsec-per-century", "j2000"],
  };
  E["julian-date"] = {
    term: "Julian date", sym: "JD", group: "time",
    short: "A continuous count of days since noon, 1 January 4713 BC (Julian calendar). Astronomers use it to avoid calendar arithmetic.",
    typical: "JD 2440400.5 = 1969-06-28 00:00; JD 2451545.0 = 2000-01-01 12:00",
    long: "Julian dates run continuously, so a time difference is just a subtraction. Dates ending in .5 fall at midnight. The page converts JD to calendar dates for labels with the standard algorithm (Meeus, Astronomical Algorithms, chapter 7), Gregorian from 1582-10-15 and Julian before. Times are in [[tdb|TDB]].",
    mercury: "The System-mode run starts at JD 2440400.5, the DE440 initial-condition epoch.",
    see: ["run-epoch", "tdb", "j2000"],
  };
  E["tdb"] = {
    term: "TDB (Barycentric Dynamical Time)", group: "time",
    short: "The time scale of solar-system dynamics and of JPL's ephemerides. All times on the page are TDB.",
    typical: "differs from TT by less than 2 ms; TT is about 69 s ahead of UTC in 2026",
    long: "TDB is the uniform time used as the independent variable for motion about the solar-system barycentre. It differs from Terrestrial Time (TT) only by tiny periodic relativistic terms. Civil time (UTC) has leap seconds and is not used; the \"Date (TDB)\" readout is therefore a dynamical date, not a wall-clock date.",
    mercury: "Using one uniform time scale keeps the passage times and fitted rates consistent with DE440.",
    see: ["julian-date", "run-epoch", "calendar-date"],
  };
  E["j2000"] = {
    term: "J2000.0", group: "time",
    short: "The standard reference epoch: 1 January 2000, 12:00 TDB (JD 2451545.0). Used for the fixed ecliptic and equinox, and as the fit's time origin.",
    long: "Frames and constants are quoted \"at J2000.0\" so they refer to one instant. The [[ecliptic-j2000]] and [[equinox-j2000]] are frozen at this epoch. The fit measures t from J2000.0, so ϖ̇ is the slope at J2000.0 and [[fit-b|B]] the longitude then. The [[rung-lab]] starts its run at J2000.0.",
    mercury: "Park et al. (2017) centred their 2000-year window on J2000.0.",
    see: ["julian-date", "ecliptic-j2000", "run-epoch"],
  };
  E["run-epoch"] = {
    term: "Run epoch", group: "time",
    short: "The instant the run starts: JD 2440400.5 TDB (1969-06-28) in System mode, the DE440 initial-condition epoch; J2000.0 in the Rung lab.",
    long: "The chart's x axis counts Julian years since this epoch, and the calendar years along its top are epoch + t. The engine integrates forward only; Park's literature values use a window centred on J2000, which is why the table notes that a forward window can differ slightly.",
    mercury: "A 1000-year run covers 1969 to 2969.",
    see: ["initial-conditions", "julian-date", "run-span"],
  };
  E["calendar-date"] = {
    term: "Date (TDB)", group: "time",
    short: "The calendar date the simulation has reached, in the TDB time scale.",
    long: "Computed for display only from the [[run-epoch]] plus the integrated time, with the standard [[julian-date]] conversion (Gregorian calendar; years can run far into the future for long spans).",
    mercury: "Watch it advance as the perihelion markers spread.",
    see: ["tdb", "julian-date", "integrated"],
  };
  E["au"] = {
    term: "Astronomical unit", sym: "au", group: "time",
    short: "The unit of length on the plate: about the Earth–Sun distance, exactly 149 597 870.700 km.",
    long: "Since 2012 the IAU defines the au as exactly 149 597 870 700 m, and DE440 uses that value. The plate's grid circles and scale bar are labelled in au; Mercury's [[semi-major-axis]] is about 0.387 au.",
    mercury: "Mercury's distance from the Sun varies between about 0.307 au (perihelion) and 0.467 au (aphelion).",
    see: ["semi-major-axis", "orbit-plate"],
  };
  E["run-span"] = {
    term: "Run span", group: "time",
    short: "How many Julian years the live run integrates before it stops.",
    typical: "1000 years by default; range 1 … 20 000",
    long: "The live run stops when it reaches the span (\"Span reached\"). Longer spans let the periodic perturbations average out and allow more [[periodic-terms]] and the [[fit-q|Q]] term in the fit. The contribution table has its own span setting ([[table-span]]).",
    mercury: "For a literature-comparable rate, run 1000 years or more.",
    see: ["table-span", "fit-model", "integrated"],
  };
  E["display-cadence"] = {
    term: "Display cadence", group: "time",
    short: "How often (in days) positions and orbital elements are sent to the page for drawing. It does not change the physics or the passages.",
    typical: "4 days by default; range 0.5 … 50; at least the WHFast step",
    long: "The engine always integrates with its own steps and samples Mercury at its internal cadence for the passage search; this setting only thins the stream used for trails, the osculating curves and the sparklines. A finer cadence gives smoother drawings at some cost in speed and memory.",
    mercury: "The fitted rate is identical for any display cadence.",
    see: ["wh-step", "osculating-varpi"],
  };
  E["sim-speed"] = {
    term: "Speed (simulated years per second)", group: "time",
    short: "How many simulated years pass per second of wall-clock time. \"max\" runs as fast as the engine can.",
    typical: "1 yr/s by default; the top speed depends on the engine and the computer (WHFast is several times faster than IAS15)",
    long: "The worker pauses between chunks to hold the requested rate; at max it runs back to back. The readout in the transport bar shows the speed actually achieved. Speed never affects the results.",
    mercury: "At 1 yr/s Mercury goes round about four times a second.",
    see: ["transport", "integrated"],
  };
  E["integrated"] = {
    term: "Integrated", group: "time",
    short: "Julian years integrated so far, out of the run span. The progress bar shows the same fraction.",
    long: "The live run advances in chunks of a fraction of a year. The reached time can exceed the request by up to one integrator step, because the engine advances in whole steps.",
    mercury: "The fit needs at least 20 passages, about 5 years.",
    see: ["run-span", "transport"],
  };

  // ----------------------------------------------------------------------------------------- fit
  E["least-squares"] = {
    term: "Least-squares fit", group: "fit",
    short: "Finds the model curve that minimises the sum of squared differences from the data. Here: the passage longitudes against time, giving ϖ̇.",
    long: "The engine fits the [[fit-model]] to the [[projected-longitude|projected passage longitudes]] by unweighted linear least squares (Householder QR). The slope coefficient is [[varpi-dot|ϖ̇]]; its uncertainty is estimated by the [[jackknife]]. The fit is refreshed at most twice a second while the run plays, and once more when the span is reached.",
    mercury: "This is the Park et al. (2017) estimator, the same code path as FerroAstro's PHY-012 nightly runs.",
    see: ["fit-model", "jackknife", "rms-residual"],
  };
  E["fit-model"] = {
    term: "Fit model", group: "fit",
    short: "The curve fitted to the passage longitudes: B + ϖ̇t, plus Qt² once the fitted span reaches 500 years, plus sinusoids at the planetary frequencies the span resolves.",
    typical: "ϖ(t) = B + ϖ̇t (+ Qt²) + Σ(Sᵢ sin νᵢt + Cᵢ cos νᵢt)",
    long: "This is Park17's model with t in Julian centuries from [[j2000|J2000.0]] TDB. [[fit-b|B]] is the longitude at J2000.0, ϖ̇ the rate, [[fit-q|Q]] a slow curvature, and the [[periodic-terms]] absorb the strongest planetary oscillations. With \"B + ϖ̇t (+ Qt², periodic terms the span resolves)\" a term is included only when its planets are on and its period is at most half the fitted span; the other choice drops the periodic terms. The exact model used is printed under \"Fit model and metadata\".",
    mercury: "Removing the periodic signal is what lets a few-century run give a usable rate: with the full model and the default fit, 200–400-year runs land about 0.1–0.2 ″/cy from the long-window 575.31 ″/cy, with a jack-knife σ of a few hundredths of ″/cy. DESIGN.md §8 E7 expects ±0.1 ″/cy only over a 1000-year window.",
    see: ["fit-b", "fit-q", "periodic-terms", "least-squares"],
  };
  E["fit-b"] = {
    term: "B (fit offset)", group: "fit",
    short: "The fitted longitude at J2000.0: the constant term of the fit model.",
    typical: "arcseconds",
    long: "Because the fit's time origin is [[j2000|J2000.0]], not the run start, B is an extrapolated value for runs that start in 1969. The chart does not use it: it draws the fitted slope through the centroid of the fitted passages instead.",
    mercury: "Only the slope ϖ̇ carries the physics; B just positions the line.",
    see: ["fit-model", "fitted-rate"],
  };
  E["fit-q"] = {
    term: "Q (quadratic term)", group: "fit",
    short: "A t² term that lets the rate change slowly over the window. Included only when the fitted span is at least 500 years.",
    typical: "arcsec per century²; small",
    long: "Over long windows the planetary orbits themselves evolve, so ϖ̇ is not exactly constant. Q absorbs that slow change; with Q in the model, ϖ̇ is the slope at J2000.0. Over shorter spans a t² term would be degenerate with the noise, so it is left out.",
    mercury: "Park17's model includes Q for their 2000-year window.",
    see: ["fit-model", "varpi-dot"],
  };
  E["periodic-terms"] = {
    term: "Periodic terms (Park17 Table 2)", group: "fit",
    short: "Sine and cosine terms at 14 frequencies built from the mean motions of Mercury, Venus, Earth, Jupiter and Saturn. They soak up the planets' regular oscillations.",
    typical: "frequencies such as nM − 2nV, 2nV, nJ, 2nS (SW92 mean motions)",
    long: "Park et al. (2017) Table 2 lists the combinations of mean motions that dominate Mercury's perihelion wiggles. The engine computes their frequencies from the Standish & Williams (1992) mean motions ([[sw92]]) and includes a term only when every planet it involves is switched on and its period is at most half the fitted span (a term that does not complete two cycles is degenerate with the polynomial). The Rung lab has no periodic terms.",
    mercury: "Without them a short-span rate can be off by several ″/cy.",
    see: ["fit-model", "synodic-period", "park17"],
  };
  E["fit-window"] = {
    term: "Fit window", group: "fit",
    short: "Which passages the live fit uses: all so far, or only those of the last 1000, 100 or 20 years.",
    long: "A shorter window follows recent behaviour but has a larger uncertainty and fewer resolvable [[periodic-terms]]. The window counts back from the current simulated time. At least 20 passages must fall in it, or the readout says it needs more.",
    mercury: "Compare \"all\" with \"last 20 yr\" to see how much short windows wobble.",
    see: ["fit-model", "jackknife"],
  };
  E["jackknife"] = {
    term: "Jack-knife σ", group: "fit",
    short: "The uncertainty shown after ±: the passages are split into consecutive blocks, the fit is redone leaving out one block at a time, and σ comes from the spread.",
    typical: "between 2 and 10 blocks (number of passages ÷ 20)",
    long: "The delete-one-block jack-knife estimates the standard error without assuming that the residuals are independent. Residuals from orbital fits are strongly correlated in time, so the [[formal-sigma]] understates the true error; the jack-knife is the honest one and is what the readout and the table use. In the table, a difference of two legs gets σ = √(σ₁² + σ₂²).",
    mercury: "The ± shrinks as the run gets longer and the periodic terms average out.",
    see: ["formal-sigma", "three-sigma", "least-squares"],
  };
  E["formal-sigma"] = {
    term: "Formal σ", group: "fit",
    short: "The textbook least-squares uncertainty, which assumes independent errors. It understates the error for correlated residuals; shown for comparison.",
    long: "The formal σ comes from the fit's covariance matrix scaled by the residual scatter. For orbital data the residuals are correlated (they are left-over periodic signal, not random noise), so it is usually too small. The readout's details list it next to the [[jackknife]] σ.",
    mercury: "A large ratio of jack-knife to formal σ means unmodelled periodic signal.",
    see: ["jackknife", "rms-residual"],
  };
  E["rms-residual"] = {
    term: "RMS residual", group: "fit",
    short: "The root-mean-square difference between the passage longitudes and the fitted model, in arcseconds.",
    long: "Small residuals mean the [[fit-model]] describes the data well. Remaining residuals are mostly periodic perturbations not included in the model. \"Subtract fitted rate\" ([[detrend]]) shows them visually.",
    mercury: "The residuals are where the fit's periodic terms (or their absence) show.",
    see: ["fit-model", "detrend", "formal-sigma"],
  };
  E["fit-passages"] = {
    term: "Passages (fitted)", group: "fit",
    short: "The dots on the chart: each perihelion passage's projected longitude, coloured by time on the shared ramp. These are the data points of the fit.",
    long: "Each dot is one [[perihelion-passage]]: its time on the x axis and its [[projected-longitude]] minus the first passage's on the y axis. The colour goes from the first passage to the latest along the same [[time-ramp]] as the perihelion markers on the plate. The readout states how many passages and which years the fit used.",
    mercury: "About 415 dots per century; their upward trend is the advance.",
    see: ["perihelion-passage", "time-ramp", "least-squares"],
  };
  E["fitted-rate"] = {
    term: "Fitted rate (line)", group: "fit",
    short: "The straight line with the fitted slope ϖ̇, drawn through the centroid of the fitted passages over the fitted span.",
    long: "Only the slope of the line comes from the fit; it is positioned through the average time and longitude of the fitted passages, because the fit's own offset [[fit-b|B]] refers to J2000.0. The label in the corner repeats the rate.",
    mercury: "The line's steepness is ϖ̇; with the full model it rises about 575″ per century (5.75″ per year).",
    see: ["varpi-dot", "fit-b", "detrend"],
  };
  E["detrend"] = {
    term: "Subtract fitted rate", group: "fit",
    short: "Subtracts ϖ̇·t from every series on the chart, leaving only the wiggles around the trend.",
    long: "With the trend removed the y range shrinks to the residual variations: the oscillations of the [[osculating-varpi]] and the scatter of the passage longitudes around the fit. The per-effect wedges are hidden in this view.",
    mercury: "Makes the planetary periodic perturbations visible even when the advance dominates.",
    see: ["rms-residual", "osculating-varpi", "fitted-rate"],
  };
  E["estimator"] = {
    term: "Estimator (Park2017)", group: "fit",
    short: "The complete recipe that turns a simulated orbit into a rate: locate passages, project on the Park17 plane, fit. The same recipe is used for every run and table leg.",
    long: "The estimator id in the metadata names its parts: Mercury relative to the Sun, the [[park17-plane]], the projected longitude, 8th-order Lagrange interpolation on the sampled states, and the [[fit-model]] with its [[jackknife]] blocks. Using one estimator for every leg of a table (CI-009) makes the differences between legs meaningful.",
    mercury: "Matches the estimator of Park et al. (2017), so the numbers are directly comparable.",
    see: ["perihelion-passage", "least-squares", "metadata"],
  };

  // --------------------------------------------------------------------------------------- table
  E["contribution-table"] = {
    term: "Per-effect contributions", group: "table",
    short: "Runs one extra integration per switched-on effect with that effect removed; each difference in rate is that effect's contribution.",
    long: "Pressing Compute runs the current set S, a Kepler-only leg and one leg S∖X for every effect X in S, all with the same engine, span, estimator and initial conditions, in parallel workers. Each row is ϖ̇(S) − ϖ̇(S∖X) ([[p-remove]]) with its σ, a literature [[reference-value]] and the [[diff]]. The footer adds the [[sum-contributions|sum]], ϖ̇(S), the [[integrator-floor]] and the [[cross-terms]].",
    mercury: "Turns one total rate into the familiar budget: Venus 277, Jupiter 154, Earth–Moon 91, GR 43, …",
    see: ["p-remove", "leg", "table-span", "budget"],
  };
  E["p-remove"] = {
    term: "P-remove", group: "table",
    short: "Contribution of effect X = rate with everything on minus rate with X switched off: ϖ̇(S) − ϖ̇(S∖X). The protocol of ADR-0014.",
    long: "Removing one effect from the full set measures what that effect adds in the presence of all the others. For a Newtonian planet it is the same as Park's \"GM set to zero\". All legs share bitwise-identical initial conditions for the remaining bodies, so the difference isolates the effect. The alternative, [[p-add]], is shown for information only.",
    mercury: "Park et al. (2017) Table 3 uses this protocol, so the rows compare directly with their values.",
    see: ["p-add", "contribution-table", "cross-terms"],
  };
  E["p-add"] = {
    term: "P-add (information only)", group: "table",
    short: "The other way round: rate with only Sun, Mercury and X, minus Kepler only. Shown for information; it differs from P-remove by interaction terms.",
    long: "P-add measures X in isolation, ϖ̇({☉, ☿} ∪ X) − ϖ̇({☉, ☿}). Because effects interact (for example Jupiter's and Saturn's pulls combine non-linearly), P-add and [[p-remove]] differ slightly. ADR-0014 makes P-remove the reported protocol; tick \"Also P-add\" to compute this extra column.",
    mercury: "The differences are of the order of the cross terms (Park17's Jupiter + Saturn cross row is 0.0411 ″/cy): small, but real.",
    see: ["p-remove", "cross-terms"],
  };
  E["table-span"] = {
    term: "Contribution span", group: "table",
    short: "How many years each leg of the table integrates, forward from the run epoch.",
    typical: "200 years by default; range 10 … 20 000",
    long: "Longer spans give smaller uncertainties and let more [[periodic-terms]] into the fit, at the cost of time. All legs use the same span. Tiny effects stay below [[three-sigma|3σ]] on typical spans: at 300–400 years each leg's jack-knife σ is about 0.01–0.02 ″/cy, so 3σ is several hundredths of ″/cy: at or above [[solar-j2]] (0.028) and far above [[lense-thirring]] (0.002) and the [[asteroids]] (0.001), whose rows stay greyed.",
    mercury: "Provisional expectations (DESIGN.md §8 E6): over 100–200 years the large planetary rows agree with Park17 to about ±2 ″/cy; over a forward 1000-year window to about ±0.05.",
    see: ["contribution-table", "three-sigma", "reference-value"],
  };
  E["reference-value"] = {
    term: "Reference value", group: "table",
    short: "The published or analytic value for each effect, for comparison. Hover a value to see its source.",
    long: "Planets use Park et al. (2017) Table 3, measured over a 2000-year window centred on J2000 on the Park17 plane. GR uses 42.9805 ″/cy (PHY-004) times (2 + 2γ − β)/3; J2 uses FerroAstro's 0.02845 ″/cy; Lense–Thirring −0.002 ″/cy; asteroids 0.0012 ″/cy. Overrides scale the J2 and spin references. The viewer runs forward from 1969, not over Park's centred window, so small differences are expected.",
    mercury: "Agreement within the stated tolerances shows the simulation reproduces the literature budget.",
    see: ["diff", "park17", "contribution-table"],
  };
  E["diff"] = {
    term: "Diff", group: "table",
    short: "This run's contribution minus the reference value, in ″/cy.",
    long: "Expect differences of order the [[jackknife]] σ plus the effect of the different window. Large differences on short spans are normal: the periodic perturbations have not averaged out.",
    mercury: "DESIGN.md §8 E6 expects the planetary diffs within about ±0.05 ″/cy each over a forward 1000-year window, and provisionally ±0.05 to ±2 ″/cy (largest for Venus, Earth–Moon and Jupiter) over 100–200 years.",
    see: ["reference-value", "jackknife", "table-span"],
  };
  E["three-sigma"] = {
    term: "Below resolution (3σ)", group: "table",
    short: "Rows whose value is within three standard deviations of zero are greyed: the run is too short to tell that effect from zero.",
    long: "With σ the combined [[jackknife]] uncertainty of the two legs, a result with |value| < 3σ cannot be told apart from zero at the 3σ level (for a Gaussian error, a true zero lands outside ±3σ only about 0.3 % of the time). Longer spans shrink σ.",
    mercury: "Lense–Thirring (−0.002 ″/cy) and the asteroids (0.001 ″/cy) are greyed on short spans.",
    see: ["jackknife", "table-span"],
  };
  E["sum-contributions"] = {
    term: "Σ contributions", group: "table",
    short: "The sum of all P-remove rows.",
    long: "If effects simply added, Σ plus the [[integrator-floor]] would equal ϖ̇(S). The small remainder is the [[cross-terms]] row.",
    mercury: "With the full model, Σ ≈ 532.3 + 42.98 + 0.028 − 0.002 ≈ 575.3 ″/cy.",
    see: ["varpi-s", "cross-terms", "integrator-floor"],
  };
  E["varpi-s"] = {
    term: "ϖ̇(S), all selected effects", group: "table",
    short: "The rate of the leg with every switched-on effect: the total that the rows break down.",
    typical: "full model ≈ 575.31 ″/cy over long spans",
    long: "The S leg uses the table's span, engine and estimator, so it can differ from the live readout, which uses the live run's span and fit window.",
    mercury: "The Park17 full-model value is 575.3100 ″/cy.",
    see: ["sum-contributions", "contribution-table"],
  };
  E["cross-terms"] = {
    term: "Interaction (cross) terms", group: "table",
    short: "ϖ̇(S) − Σ − ϖ̇(Kepler): what is left because effects do not add exactly. Not an error.",
    typical: "Park17 Jupiter + Saturn cross row: 0.0411 ″/cy",
    long: "Perturbations interact: Jupiter perturbs Saturn's orbit, which changes Saturn's pull on Mercury, and so on. Removing two effects one at a time therefore does not remove their combined action. Park et al. (2017) list the Jupiter + Saturn cross term at 0.0411 ″/cy; the row shows it for scale.",
    mercury: "A small, physical residual that confirms the bookkeeping rather than a numerical problem.",
    see: ["p-remove", "p-add", "sum-contributions"],
  };
  E["leg"] = {
    term: "Leg", group: "table",
    short: "One independent integration in the table (S, Kepler only, or S without one effect). The column shows each leg's short IC hash and engine.",
    long: "Legs run in parallel in their own workers with the same engine, span, estimator and initial conditions, differing only by the switched-off effect. A progress bar shows each running leg; an error is shown in place with the message on hover.",
    mercury: "A table for the full model runs 12 legs.",
    see: ["ic-hash", "contribution-table"],
  };
  E["ic-hash"] = {
    term: "IC hash", group: "table",
    short: "A fingerprint of a run's initial state. The table shows its first characters for each leg.",
    long: "The hash covers the leg's own body set, so legs with different bodies have different hashes even though the shared bodies start from identical states (NUM-013). The full hash is in the metadata and the CSV export.",
    mercury: "Proves that every leg started from the same DE440 state.",
    see: ["leg", "initial-conditions", "metadata"],
  };
  E["wedges"] = {
    term: "Per-effect wedges", group: "table",
    short: "Coloured fans on the chart: the table's contributions stacked as slopes from the Kepler floor, positive ones upwards, negative ones downwards and hatched. Indicative only.",
    long: "Once the table has been computed for the current settings, each contribution is drawn as a wedge whose slope is that rate, starting at the first passage. Positive ones stack upwards, largest at the bottom; negative ones (Lense–Thirring, a Sun-centred [[gamma0]]) stack downwards below the floor, hatched, and a dashed ray then marks the net sum. The wedges come from the table's own span, so they show proportions rather than a fit to this run. Hidden when \"Subtract fitted rate\" is on.",
    mercury: "Shows at a glance that Venus and Jupiter dominate and GR is a thin slice.",
    see: ["contribution-table", "budget"],
  };
  E["budget"] = {
    term: "Rate budget strip", group: "table",
    short: "Every contribution to one scale in one bar, each in its effect's colour: positive ones right of the zero mark, negative ones left of it, hatched.",
    long: "Each segment's width is its share of the sum of the magnitudes of all contributions, with a sliver of minimum width so even the smallest rows show. Negative contributions, such as Lense–Thirring or a Sun-centred [[gamma0]], sit left of the zero mark and are hatched so the sign does not rest on colour alone. The key gives the signed values in ″/cy; a [[comparison-model]] row says so.",
    mercury: "Venus is about half of the whole bar.",
    see: ["contribution-table", "wedges"],
  };
  E["export-csv"] = {
    term: "Export CSV", group: "table",
    short: "Downloads the table as a CSV file, with the run metadata as # comment lines.",
    long: "Columns: effect, protocol, rate and jack-knife σ, reference and its source, difference, P-add and σ, leg IC hash; then the sum, ϖ̇(S), ϖ̇(Kepler only) and the cross-term row.",
    mercury: "Keeps every number together with the [[metadata]] that defines it (PHY-015).",
    see: ["metadata", "contribution-table"],
  };

  // ---------------------------------------------------------------------------------------- data
  E["de440"] = {
    term: "DE440", group: "data",
    short: "JPL's Development Ephemeris 440 (Park et al. 2021): the planetary ephemeris whose initial conditions and constants this engine uses.",
    typical: "covers 1550–2650",
    long: "DE440 is fitted to modern observations (radar, spacecraft ranging, lunar laser ranging). FerroAstro takes its masses ([[gm|GM]]), the au, the solar J₂, radius and spin, and the initial states of the Sun and planets at JD 2440400.5, from its [[header-440|header file]]. The integration then proceeds with FerroAstro's own force models and integrators.",
    mercury: "Starting from DE440 puts Mercury and the planets where they really were in 1969.",
    see: ["header-440", "initial-conditions", "gm"],
  };
  E["header-440"] = {
    term: "header.440", group: "data",
    short: "The DE440 header file with its constants and initial conditions. It is embedded in the wasm at build time and never committed to the repository.",
    long: "The file is fetched from JPL, checked against a pinned SHA-256 and included by the engine's embed-header build feature (FerroAstro #22). No value from it lives in the source tree, which is why the page shows the DE440 J₂ only as the engine reports it. A module built without the feature refuses System mode and the Rung lab with FA-E0303.",
    mercury: "Supplies the 1969 state vectors and the constants every run uses.",
    see: ["de440", "initial-conditions", "error-codes"],
  };
  E["initial-conditions"] = {
    term: "Initial conditions", group: "data",
    short: "Where every body is and how it moves at the start: the DE440 state at JD 2440400.5 TDB (1969-06-28), in ICRF axes about the barycentre.",
    long: "System mode reads the DE440 GROUP 1041 states at the epoch JDEPOC. Removing a body from a run leaves every other body's starting state bitwise identical (NUM-013), which is what makes [[p-remove]] differences clean. The Rung lab instead starts from the [[sw92]] mean elements at J2000.0.",
    mercury: "The same start state for every run and table leg: differences come only from the physics switched.",
    see: ["de440", "run-epoch", "ic-hash"],
  };
  E["gm"] = {
    term: "GM (gravitational parameter)", group: "data",
    short: "The product of the gravitational constant and a body's mass. Ephemerides give GM directly because it is known far more precisely than G or the mass separately.",
    long: "Each body's pull is set by its GM from DE440. For Mars and the outer planets the value is for the planet plus its moons (the \"system\"); the Earth and Moon are combined in the [[emb]]. Setting a GM to zero is equivalent to removing the body, which is what the planet toggles do.",
    mercury: "The orbital elements use μ = GM☉ + GM☿, the two-body value.",
    see: ["de440", "planetary-perturbations"],
  };
  E["park17"] = {
    term: "Park et al. (2017)", group: "data",
    short: "\"Precession of Mercury's perihelion from ranging to the MESSENGER spacecraft\" (AJ 153, 121): the source of the reference frame, the estimator and the reference values.",
    long: "Park and colleagues derived Mercury's perihelion budget from MESSENGER ranging and the JPL ephemeris dynamical model: planets ≈ 532.3 ″/cy, GR 42.98, solar J₂ ≈ 0.03, Lense–Thirring −0.002, totalling 575.31 ″/cy on a fixed mean-orbit plane ([[park17-plane]]), with a 2000-year window centred on J2000. Their Table 2 frequencies form the fit's [[periodic-terms]]; their Table 3 provides the planetary reference values.",
    mercury: "The viewer is built to reproduce this budget.",
    see: ["park17-plane", "periodic-terms", "reference-value"],
  };
  E["clemence"] = {
    term: "Clemence (1947)", group: "data",
    short: "G. M. Clemence's classic determination of Mercury's observed perihelion motion: 5599.74 ± 0.41 ″/cy against the equinox of date.",
    long: "Clemence's review (Reviews of Modern Physics 19, 361) summarised the observed and Newtonian-predicted motions, leaving about 43 ″/cy for relativity. His figure includes the [[general-precession]] of about 5025 ″/cy. See the [[frame-caveat]] for why it cannot be compared directly with an inertial simulation.",
    mercury: "The observed number most often quoted for Mercury.",
    see: ["frame-caveat", "general-precession"],
  };
  E["sw92"] = {
    term: "Standish & Williams (1992)", group: "data",
    short: "Mean orbital elements and mean motions of the planets at J2000. They give the Rung lab's starting orbit and the fit's periodic frequencies.",
    long: "Standish and Williams (1992) tabulated mean Keplerian elements and mean motions of the planets for approximate positions. The [[rung-lab]] starts Mercury from these J2000 mean elements on the J2000 mean ecliptic; the fit builds its [[periodic-terms]] from their Table 2a mean motions.",
    mercury: "A good average orbit for the single-body tests.",
    see: ["rung-lab", "periodic-terms"],
  };
  E["reference-ids"] = {
    term: "ADR, PHY and other ids", group: "data",
    short: "Codes such as ADR-0014 or PHY-004 point to FerroAstro's design decisions and requirements, where each definition and number is documented.",
    long: "ADRs (architecture decision records) fix definitions: ADR-0011 the frame and observable, ADR-0012 the SR rungs, ADR-0014 the P-remove protocol. PHY-, NUM- and CI- ids are requirements in docs/requirements; ids such as PHY004-GR-RATE name test vectors in tests/vectors. Park17 Table numbers refer to [[park17]].",
    mercury: "Every reference number on the page can be traced through these ids.",
    see: ["park17", "metadata"],
  };
  E["metadata"] = {
    term: "Run metadata (PHY-015)", group: "data",
    short: "The facts needed to interpret a rate: engine, force model, gauge, frame, epoch, fit model, estimator, constants version and IC hash. Every rate is shown with it.",
    long: "PHY-015 requires a rate to be reported with its metadata or not at all. The line under the physics controls shows the gauge, IC hash and constants version (hover it for the full model and IC source); \"Fit model and metadata\" and the table header show the rest. The CSV export carries all of it.",
    mercury: "Two rates are comparable only if their frame, window and estimator match.",
    see: ["gauge", "estimator", "constants-version", "ic-hash"],
  };
  E["constants-version"] = {
    term: "Constants version", group: "data",
    short: "Which set of physical constants (DE440, from the embedded header) the engine used, so a result can be reproduced exactly.",
    long: "Reported in the metadata line, the fit details and the table header. Since #22 the DE440 constants come from the embedded [[header-440]], checked against its SHA-256 pin at run time.",
    mercury: "Changing GM values would change every rate; recording the version prevents silent mismatches.",
    see: ["metadata", "header-440"],
  };
  E["scenario-toml"] = {
    term: "Scenario TOML", group: "data",
    short: "The run's full specification (bodies, forces, initial conditions, integrator, span) as a TOML text file. Run it with the CLI to reproduce the result.",
    long: "Copied to the clipboard (or downloaded if the clipboard is unavailable). Run it with `ferroastro scenario run <file>`: the same engine code gives the same numbers natively.",
    mercury: "Makes any view on this page reproducible outside the browser.",
    see: ["wasm-engine", "metadata"],
  };
  E["nominal-run"] = {
    term: "Nominal run", group: "data",
    short: "A run with the real DE440 J₂ and solar spin, IAS15 ε ≤ 10⁻⁹ and, with conformal gravity on, its fit coefficients. Anything else is non-nominal and labelled.",
    long: "Only nominal runs describe the real solar system at full accuracy. The metadata field \"nominal\" records it; the header badge and readout show \"Exaggerated physics\" or \"non-nominal\" otherwise ([[physical-exaggeration]]).",
    mercury: "Literature comparisons need a nominal run.",
    see: ["physical-exaggeration", "metadata"],
  };
  E["error-codes"] = {
    term: "Error codes (FA-E…)", group: "data",
    short: "Engine messages start with a code: FA-E0002 a value out of range, FA-E0303 built without embed-header, FA-E0603 too few passages for a fit, FA-E1201 an engine that cannot model a switch.",
    long: "Codes make messages searchable in the FerroAstro documentation. The error banner shows the full text, which also says how to fix the problem. A fit refusal (too few passages) is not fatal: the run continues and the fit appears once there are 20 passages.",
    mercury: "None of these change the physics; they explain why a run or a fit could not start.",
    see: ["engine-refusal", "header-440", "messages"],
  };
  E["messages"] = {
    term: "Status and error messages", group: "data",
    short: "The status line says what the engine is doing; the red banner explains anything it refused, usually with an FA-E code and a fix.",
    long: "Typical status lines: \"Loading the engine…\", \"Running System mode with IAS15.\", \"Span reached\" (the run stopped at its span), \"Configuration refused.\" (see the banner). \"Needs at least 20 perihelion passages\" means the fit is waiting for data. If the engine traps, it is restarted with the same settings, paused; repeated failures stop the automatic restart.",
    mercury: "Most messages resolve by waiting a few simulated years or choosing another engine.",
    see: ["error-codes", "engine-refusal"],
  };

  // ------------------------------------------------------------------------------------ controls
  E["system-mode"] = {
    term: "System mode", group: "controls",
    short: "The full N-body simulation: Sun, Mercury and whichever planets and effects are switched on, from the DE440 1969 state.",
    long: "The default mode. Every planet is a real integrated body, so its perturbation of Mercury and its own motion are both modelled. Switch to the [[rung-lab]] to study the central force alone.",
    mercury: "Where the ≈ 575 ″/cy total and its budget live.",
    see: ["rung-lab", "initial-conditions"],
  };
  E["presets"] = {
    term: "Presets", group: "controls",
    short: "One-click toggle sets: Kepler only (no perturbations), Planets only, GR only, Full model (planets, J2, GR, Lense–Thirring) and Full model + CG (adds the conformal-gravity comparison model).",
    long: "[[preset-kepler]] checks the [[integrator-floor]]; [[preset-planets]] gives the Newtonian ≈ 532 ″/cy; [[preset-gr]] gives ≈ 42.98 ″/cy; [[preset-full]] gives ≈ 575.31 ″/cy; [[preset-full-cg]] adds the [[comparison-model]] on top. In the Rung lab the presets choose a rung instead (Planets only is unavailable).",
    mercury: "Compare the presets to see the classic split: Newtonian planets versus relativity.",
    see: ["preset-kepler", "preset-planets", "preset-gr", "preset-full", "preset-full-cg"],
  };
  E["preset-kepler"] = {
    term: "Kepler only", group: "controls",
    short: "Sun and Mercury with Newtonian gravity only. Expect ϖ̇ ≈ 0; any rate is numerical error.",
    typical: "|ϖ̇| ≤ 0.001 ″/cy over 100 years",
    long: "Switches every effect off (mask 0). In the Rung lab it selects the Newton rung with no J2 or Lense–Thirring. See [[kepler-problem]] and [[integrator-floor]].",
    mercury: "The baseline against which every advance is measured.",
    see: ["kepler-problem", "integrator-floor"],
  };
  E["preset-planets"] = {
    term: "Planets only", group: "controls",
    short: "All seven other planets with Newtonian gravity, no relativity or solar effects.",
    typical: "≈ 532.3 ″/cy (Park17)",
    long: "Switches on Venus, the Earth–Moon barycentre, Mars, Jupiter, Saturn, Uranus and Neptune. AUTO resolves to WHFast. Not available in the Rung lab.",
    mercury: "What Newtonian mechanics predicts: about 43 ″/cy short of the full value.",
    see: ["planetary-perturbations", "whfast"],
  };
  E["preset-gr"] = {
    term: "GR only", group: "controls",
    short: "Sun and Mercury with the Sun's 1PN relativistic field and no planets.",
    typical: "≈ 42.98 ″/cy",
    long: "Switches on only [[gr-1pn]]. With β = γ = 1, AUTO resolves to WHFast with the GR effective potential. In the Rung lab it selects the GR rung.",
    mercury: "Isolates Einstein's 43 ″/cy.",
    see: ["gr", "gr-1pn"],
  };
  E["preset-full"] = {
    term: "Full model", group: "controls",
    short: "Planets, solar J2, GR 1PN and Lense–Thirring (bits 0–9). Asteroids and EIH stay off.",
    typical: "≈ 575.31 ″/cy over a long forward window",
    long: "The viewer's default and its closest match to Park17's budget. Lense–Thirring forces IAS15. In the Rung lab it selects the GR rung with J2 and Lense–Thirring.",
    mercury: "Park17: 575.3100 ″/cy; FerroAstro's PHY-014 total with EIH, asteroids and KBOs: 575.3093.",
    see: ["varpi-dot", "ias15"],
  };
  E["preset-full-cg"] = {
    term: "Full model + CG", group: "controls",
    short: "The Full model plus conformal gravity (bit 12), a comparison model outside the GR ladder, with the current γ0, γ*, κ and centring.",
    typical: "≈ 575.31 ″/cy plus the CG Gauss reference: ≈ 0 (uniform γ0, fit values) or ≈ −0.18 ″/cy (Sun-centred)",
    long: "Use it to see the comparison model next to the full GR model. With the defaults (fit values, uniform γ0) the contribution is about 10⁻¹¹ ″/cy, so the table row stays greyed; switch the [[gamma0-centring]] to Sun-centred to see the γ0 term. EIH and Lense–Thirring are velocity-dependent, so a uniform γ0 is then not exactly zero, only far below resolution. In the Rung lab it selects the GR rung with J2, Lense–Thirring and conformal gravity.",
    mercury: "Its CG row is never part of the 575.31 ″/cy budget.",
    see: ["preset-full", "conformal-gravity", "comparison-model"],
  };
  E["effects-toggles"] = {
    term: "Effects (toggles)", group: "controls",
    short: "Switch each perturbation on or off. The figure on the right is its expected contribution to ϖ̇; \"IAS15 only\" means WHFast cannot model it.",
    long: "Any change restarts the live run (after asking if more than 10 s of integration would be lost). The expected values are approximate literature numbers on the Park17 plane; the table computes the actual contributions. Below a dashed line, [[conformal-gravity]] is a [[comparison-model]], not part of the GR ladder: its figure is the engine's [[cg-gauss-reference|Gauss reference]] for the current settings. In the Rung lab only Solar J2, Lense–Thirring and conformal gravity are available.",
    mercury: "Together the switched-on effects make up the advance shown in the readout.",
    see: ["planetary-perturbations", "gr-1pn", "solar-j2", "lense-thirring", "asteroids", "eih", "conformal-gravity"],
  };
  E["asteroid-count"] = {
    term: "Asteroid count", group: "controls",
    short: "How many asteroids the Big asteroids toggle adds, taken in order from DE440's list of 343.",
    typical: "16 by default; range 1 … 343",
    long: "Only active with the [[asteroids]] toggle on (IAS15). More asteroids cost more time for an effect of about a thousandth of an ″/cy.",
    mercury: "Park17 finds 0.0012 ″/cy for all 343.",
    see: ["asteroids"],
  };
  E["fit-model-choice"] = {
    term: "Fit model (choice)", group: "controls",
    short: "Whether the live fit includes the periodic terms the span resolves. The Qt² term is added either way once the fitted span reaches 500 years.",
    long: "\"B + ϖ̇t (+ Qt², periodic terms the span resolves)\" is the Park17-style model; \"B + ϖ̇t (+ Qt²), no periodic terms\" shows why short-span rates wobble. See [[fit-model]].",
    mercury: "With the periodic terms, a few centuries already give a rate close to the long-window value.",
    see: ["fit-model", "periodic-terms", "fit-q"],
  };
  E["display-exaggeration"] = {
    term: "Display exaggeration", group: "controls",
    short: "Rotates only the drawn Mercury ellipse and perihelion markers by (k − 1)·Δϖ, so the advance becomes visible. Never changes the physics, the chart or the table.",
    typical: "×1 … ×10⁵ (logarithmic slider)",
    long: "The real advance is about 1.6° per thousand years, invisible at plate scale. With k = 1000 the drawn perihelion turns 1000 times further than computed. While k ≠ 1 a badge is drawn into the canvas itself, so it appears in any screenshot. Compare [[physical-exaggeration]], which changes the simulated physics.",
    mercury: "Try ×1000 on a 100-year run to watch the perihelion fan out.",
    see: ["delta-varpi", "osculating-ellipse", "physical-exaggeration"],
  };
  E["perihelion-markers"] = {
    term: "Perihelion markers", group: "controls",
    short: "Ticks at the position of each recorded perihelion passage (up to 48 spread over the run), coloured from the first to the latest, with lines from the Sun to the first and latest.",
    long: "Each tick is drawn at the passage's position on the [[ecliptic-j2000|J2000 ecliptic]], pointing away from the Sun, coloured on the [[time-ramp]]. The arc between the first and latest lines is labelled with [[delta-varpi|Δϖ]]. With many passages the markers are thinned evenly so they span the whole run.",
    mercury: "The fan of markers is the perihelion advance made visible (with the display exaggeration if needed).",
    see: ["perihelion-passage", "time-ramp", "display-exaggeration"],
  };
  E["time-ramp"] = {
    term: "Time colour ramp", group: "controls",
    short: "The colour scale from the first perihelion passage to the latest, shared by the plate's markers and the chart's dots.",
    long: "The labels at each end give the calendar years of the first and latest passages. The same colour means the same time on both views, so you can match a marker on the plate with its dot on the chart.",
    mercury: "Early passages sit at one end of the fan, late ones at the other.",
    see: ["perihelion-markers", "fit-passages"],
  };
  E["trails"] = {
    term: "Trails", group: "controls",
    short: "Fading lines behind each body showing about its last orbit (two for Mercury).",
    long: "Drawn from the display samples ([[display-cadence]]), heliocentric, top-down on the J2000 ecliptic. Purely visual.",
    mercury: "Mercury's trail shows its eccentric orbit closing on itself.",
    see: ["orbit-plate", "display-cadence"],
  };
  E["orbit-plate"] = {
    term: "Orbit plate", group: "controls",
    short: "Top-down view of the orbits on the J2000 ecliptic, Sun at the centre. Grid circles and the scale bar are in au; the dashed line points to the J2000 equinox.",
    long: "Positions are heliocentric and rotated into the [[ecliptic-j2000]]. Bodies that are switched off are absent and struck through in the legend. Wheel or pinch to zoom, drag to pan, double-click to reset; with the plate focused, + and − zoom, the arrow keys pan and 0 resets. Zoom presets: Mercury (±0.55 au), Inner system (±1.7 au) and Full system (±32 au).",
    mercury: "Mercury's orbit, its osculating ellipse and the perihelion markers are drawn on top.",
    see: ["ecliptic-j2000", "equinox-j2000", "au", "zoom-presets"],
  };
  E["zoom-presets"] = {
    term: "Zoom presets", group: "controls",
    short: "Mercury (±0.55 au), Inner system (±1.7 au) or Full system (±32 au, out to Neptune).",
    long: "Each preset centres the plate on the Sun. Manual zooming or panning deselects the preset.",
    mercury: "The Mercury preset makes the perihelion markers easiest to see.",
    see: ["orbit-plate", "au"],
  };
  E["varpi-chart"] = {
    term: "ϖ(t) chart", group: "controls",
    short: "Perihelion longitude against time. x: Julian years since the run epoch (calendar years on top). y: arcseconds relative to the first passage.",
    long: "Shows the fitted [[fit-passages|passage dots]], the [[fitted-rate]] line, the thin [[osculating-varpi]] and optionally [[varpi-ecl]] and the per-effect [[wedges]]. The y label reads ϖ − ϖ₀ (″), or ϖ − ϖ₀ − ϖ̇t (″) when the fitted rate is subtracted ([[detrend]]).",
    mercury: "The upward slope of the dots is the perihelion advance.",
    see: ["varpi", "fit-passages", "fitted-rate", "detrend"],
  };
  E["transport"] = {
    term: "Playback", group: "controls",
    short: "Play or pause the live run, restart it from the epoch, and set the simulation speed. The readouts show the date, years integrated, achieved speed and engine.",
    long: "Playing never changes results, only how fast they arrive. Restart re-runs the current settings from t = 0. When reduced motion is requested by the system, the run starts paused.",
    mercury: "Press Play and watch the passage count, Δϖ and the readout converge.",
    see: ["sim-speed", "integrated", "calendar-date"],
  };
  E["colour-theme"] = {
    term: "Colour theme", group: "controls",
    short: "Auto follows the system setting; Light and Dark force one. The choice is remembered in this browser only.",
    long: "Every colour, including those on the canvases, comes from the same set of tokens, so both themes keep the effect colours consistent between the toggles, chart, plate and table.",
    mercury: "No effect on the physics.",
    see: ["orbit-plate"],
  };

  for (const id of Object.keys(E)) Object.freeze(E[id]);

  root.FA_GLOSSARY = Object.freeze({ groups: Object.freeze(groups), entries: Object.freeze(E) });
})(typeof globalThis !== "undefined" ? globalThis : this);
