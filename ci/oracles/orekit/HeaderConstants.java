// SPDX-FileCopyrightText: 2026 Proper-Time Physics Institute
// SPDX-License-Identifier: Apache-2.0
//
// Oracle (b) of the live cross-check SYSTEM-HEADER-LIVE-CROSSCHECK-MAX-ULP (#22): the DE440
// header constants as Orekit 13.1.9 reads them from a JPL binary DE file, through the documented
// API only: DataProvidersManager + DirectoryCrawler and JPLEphemeridesLoader.getLoadedConstant.
// The file is DE-440-ephemerides/lnxp1990.440 of the pinned orekit-data snapshot, kept outside
// the data tree by the Dockerfile (so StageB still sees only the mounted ephemeris).
//
// Usage: java -cp '/opt/orekit/lib/*:/opt/orekit/classes' HeaderConstants OREKIT_DATA DE_DIR OUT
//        NAME [NAME ...]
// OUT: JSON {"reader": ..., "constants": {NAME: {"repr": ..., "bits": "<16 hex>"}}}. A name the
// file does not define (Orekit returns NaN) is an error (exit status 1). Never committed.
import java.io.File;
import java.io.IOException;
import java.io.PrintWriter;
import java.nio.charset.StandardCharsets;
import org.orekit.bodies.JPLEphemeridesLoader;
import org.orekit.data.DataContext;
import org.orekit.data.DataProvidersManager;
import org.orekit.data.DirectoryCrawler;

public final class HeaderConstants {
  private HeaderConstants() {}

  public static void main(String[] args) throws IOException {
    if (args.length < 4) {
      System.err.println("usage: HeaderConstants OREKIT_DATA DE_DIR OUT NAME [NAME ...]");
      System.exit(2);
    }
    DataProvidersManager manager = DataContext.getDefault().getDataProvidersManager();
    manager.addProvider(new DirectoryCrawler(new File(args[0])));
    manager.addProvider(new DirectoryCrawler(new File(args[1])));
    JPLEphemeridesLoader loader =
        new JPLEphemeridesLoader(
            JPLEphemeridesLoader.DEFAULT_DE_SUPPORTED_NAMES,
            JPLEphemeridesLoader.EphemerisType.SUN);
    StringBuilder json = new StringBuilder();
    json.append("{\n  \"reader\": \"Orekit 13.1.9 JPLEphemeridesLoader.getLoadedConstant\",\n");
    json.append("  \"constants\": {\n");
    int missing = 0;
    for (int i = 3; i < args.length; i++) {
      String name = args[i];
      double v = loader.getLoadedConstant(name);
      if (Double.isNaN(v)) {
        System.err.println("constant " + name + " is not defined in the DE file");
        missing++;
      }
      json.append("    \"").append(name).append("\": {\"repr\": \"")
          .append(Double.toString(v)).append("\", \"bits\": \"")
          .append(String.format("%016x", Double.doubleToRawLongBits(v))).append("\"}")
          .append(i + 1 < args.length ? ",\n" : "\n");
    }
    json.append("  }\n}\n");
    try (PrintWriter out = new PrintWriter(new File(args[2]), StandardCharsets.UTF_8)) {
      out.print(json);
    }
    if (missing > 0) {
      System.exit(1);
    }
  }
}
