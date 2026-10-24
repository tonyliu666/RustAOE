import ghidra.app.script.GhidraScript;
import ghidra.app.decompiler.DecompInterface;
import ghidra.app.decompiler.DecompileResults;
import ghidra.program.model.listing.Function;
import ghidra.program.model.listing.FunctionIterator;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileWriter;
import java.io.PrintWriter;

public class DumpAllC extends GhidraScript {
    @Override
    public void run() throws Exception {
        String[] args = getScriptArgs();
        File out = new File(args[0]);
        DecompInterface d = new DecompInterface();
        d.openProgram(currentProgram);
        FunctionIterator it = currentProgram.getFunctionManager().getFunctions(true);
        int ok = 0, failed = 0;
        try (PrintWriter pw = new PrintWriter(new BufferedWriter(new FileWriter(out)))) {
            for (Function f : it) {
                if (monitor.isCancelled()) break;
                DecompileResults r = d.decompileFunction(f, 60, monitor);
                pw.println("// ===== " + f.getName() + " @ " + f.getEntryPoint());
                if (r.decompileCompleted()) {
                    pw.println(r.getDecompiledFunction().getC());
                    ok++;
                } else {
                    pw.println("// decompile failed: " + r.getErrorMessage());
                    failed++;
                }
            }
        }
        d.dispose();
        println("DUMP_DONE ok=" + ok + " failed=" + failed);
    }
}
