// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// Oracle stage B (REF-006, WP17): a massless particle in the field of a central body and third
// bodies whose positions come from a JPL binary DE file, propagated by Orekit 13.1.9 with
// Hipparchus 4.0.3 through their documented API: DataProvidersManager + DirectoryCrawler,
// CelestialBodyFactory, CartesianOrbit, NumericalPropagator, NewtonianAttraction,
// ThirdBodyAttraction, DormandPrince853Integrator and a fixed-step handler.
//
// Usage: java -cp '/opt/orekit/lib/*:/opt/orekit/classes' StageB PARAMS.properties OUT.csv
// PARAMS keys: orekit_data, jpl_dir, epoch_jd_tdb, span_days, cadence_days, central, particle,
// field (comma-separated Orekit body names), min_step_s, max_step_s, abs_tol_m, rel_tol.
// OUT: one row per output time, `t_days,naif,x_km,y_km,z_km,vx_km_s,vy_km_s,vz_km_s` relative to
// the central body (ICRF axes), Java's shortest round-trip doubles; then lines `#gm <body> <m3/s2>`
// with the GMs Orekit read from the DE file and `#rotation_to_icrf_rad <angle>` (integration frame).
import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Properties;
import org.hipparchus.geometry.euclidean.threed.Vector3D;
import org.hipparchus.ode.nonstiff.DormandPrince853Integrator;
import org.orekit.bodies.CelestialBody;
import org.orekit.bodies.CelestialBodyFactory;
import org.orekit.data.DataContext;
import org.orekit.data.DataProvidersManager;
import org.orekit.data.DirectoryCrawler;
import org.orekit.forces.gravity.NewtonianAttraction;
import org.orekit.forces.gravity.ThirdBodyAttraction;
import org.orekit.frames.Frame;
import org.orekit.frames.FramesFactory;
import org.orekit.orbits.CartesianOrbit;
import org.orekit.orbits.OrbitType;
import org.orekit.propagation.SpacecraftState;
import org.orekit.propagation.numerical.NumericalPropagator;
import org.orekit.time.AbsoluteDate;
import org.orekit.time.TimeScale;
import org.orekit.time.TimeScalesFactory;
import org.orekit.utils.PVCoordinates;

public final class StageB {
  private static final double DAY = 86400.0;
  private static final double J2000_JD = 2451545.0;

  private StageB() {}

  private static int naif(String name) {
    switch (name) {
      case "Sun": return 10;
      case "Mercury": return 1;
      case "Venus": return 2;
      case "Earth": return 399;
      case "Moon": return 301;
      case "Mars": return 4;
      case "Jupiter": return 5;
      case "Saturn": return 6;
      case "Uranus": return 7;
      case "Neptune": return 8;
      case "Pluto": return 9;
      default: throw new IllegalArgumentException("unknown body " + name);
    }
  }

  public static void main(String[] args) throws IOException {
    Properties p = new Properties();
    try (InputStream in = new FileInputStream(args[0])) {
      p.load(in);
    }
    DataProvidersManager manager = DataContext.getDefault().getDataProvidersManager();
    manager.addProvider(new DirectoryCrawler(new File(p.getProperty("orekit_data"))));
    manager.addProvider(new DirectoryCrawler(new File(p.getProperty("jpl_dir"))));

    TimeScale tdb = TimeScalesFactory.getTDB();
    AbsoluteDate j2000Tdb = new AbsoluteDate(2000, 1, 1, 12, 0, 0.0, tdb);
    double epochJd = Double.parseDouble(p.getProperty("epoch_jd_tdb"));
    AbsoluteDate epoch = j2000Tdb.shiftedBy((epochJd - J2000_JD) * DAY);
    double spanDays = Double.parseDouble(p.getProperty("span_days"));
    double cadence = Double.parseDouble(p.getProperty("cadence_days")) * DAY;

    CelestialBody central = CelestialBodyFactory.getBody(p.getProperty("central"));
    CelestialBody particle = CelestialBodyFactory.getBody(p.getProperty("particle"));
    Frame frame = central.getInertiallyOrientedFrame();
    PVCoordinates pv0 = particle.getPVCoordinates(epoch, frame);
    double mu = central.getGM();

    DormandPrince853Integrator integrator = new DormandPrince853Integrator(
        Double.parseDouble(p.getProperty("min_step_s")),
        Double.parseDouble(p.getProperty("max_step_s")),
        Double.parseDouble(p.getProperty("abs_tol_m")),
        Double.parseDouble(p.getProperty("rel_tol")));
    NumericalPropagator prop = new NumericalPropagator(integrator);
    prop.setOrbitType(OrbitType.CARTESIAN);
    prop.addForceModel(new NewtonianAttraction(mu));
    List<String> gms = new ArrayList<>();
    gms.add("#gm " + central.getName() + " " + Double.toString(mu));
    for (String name : p.getProperty("field").split(",")) {
      CelestialBody b = CelestialBodyFactory.getBody(name.trim());
      prop.addForceModel(new ThirdBodyAttraction(b));
      gms.add("#gm " + b.getName() + " " + Double.toString(b.getGM()));
    }
    prop.setInitialState(new SpacecraftState(new CartesianOrbit(pv0, frame, epoch, mu)));

    // Output axes: ICRF. The integration frame's orientation is whatever Orekit gives the central
    // body's inertially oriented frame (its rotation from ICRF is printed); the states are
    // rotated to ICRF axes and taken relative to the central body for the output.
    Frame icrf = FramesFactory.getICRF();
    double rotation = frame.getTransformTo(icrf, epoch).getRotation().getAngle();
    int pid = naif(p.getProperty("particle"));
    try (PrintWriter out = new PrintWriter(args[1], StandardCharsets.UTF_8)) {
      prop.getMultiplexer().add(cadence, s -> {
        double t = s.getDate().durationFrom(epoch);
        long k = Math.round(t / cadence);
        PVCoordinates pvp = s.getPVCoordinates(icrf);
        PVCoordinates pvc = central.getPVCoordinates(s.getDate(), icrf);
        Vector3D r = pvp.getPosition().subtract(pvc.getPosition());
        Vector3D v = pvp.getVelocity().subtract(pvc.getVelocity());
        out.println(Double.toString(k * cadence / DAY) + "," + pid + ","
            + Double.toString(r.getX() / 1000.0) + "," + Double.toString(r.getY() / 1000.0) + ","
            + Double.toString(r.getZ() / 1000.0) + "," + Double.toString(v.getX() / 1000.0) + ","
            + Double.toString(v.getY() / 1000.0) + "," + Double.toString(v.getZ() / 1000.0)
            + ",dt=" + Double.toString(t - k * cadence));
      });
      prop.propagate(epoch.shiftedBy(spanDays * DAY));
      for (String g : gms) {
        out.println(g);
      }
      out.println("#rotation_to_icrf_rad " + Double.toString(rotation));
    }
  }
}
