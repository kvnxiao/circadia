$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class ActiveDisplayRates {
    [DllImport("user32.dll")]
    public static extern int GetDisplayConfigBufferSizes(uint flags, out uint paths, out uint modes);
    [DllImport("user32.dll")]
    public static extern int QueryDisplayConfig(uint flags, ref uint paths, IntPtr pathData, ref uint modes, IntPtr modeData, IntPtr topology);
    public static void Read() {
        uint paths, modes;
        int error = GetDisplayConfigBufferSizes(2, out paths, out modes);
        if (error != 0) throw new Exception("GetDisplayConfigBufferSizes: " + error);
        // DISPLAYCONFIG_PATH_INFO is 72 bytes; DISPLAYCONFIG_MODE_INFO is 64 bytes.
        IntPtr pathData = Marshal.AllocHGlobal(checked((int)paths * 72));
        IntPtr modeData = IntPtr.Zero;
        try {
            modeData = Marshal.AllocHGlobal(checked((int)modes * 64));
            error = QueryDisplayConfig(2, ref paths, pathData, ref modes, modeData, IntPtr.Zero);
            if (error != 0) throw new Exception("QueryDisplayConfig: " + error);
            for (int i = 0; i < paths; i++) {
                IntPtr path = IntPtr.Add(pathData, i * 72);
                int index = Marshal.ReadInt32(path, 32);
                int numerator = Marshal.ReadInt32(path, 48), denominator = Marshal.ReadInt32(path, 52);
                if (denominator <= 0) throw new Exception("Invalid path refresh denominator");
                Console.WriteLine("Active path {0}: {1}/{2} = {3:F6} Hz", i, numerator, denominator, (double)numerator / denominator);
                if (index >= 0 && index < modes) {
                    IntPtr mode = IntPtr.Add(modeData, index * 64);
                    if (Marshal.ReadInt32(mode) == 2) {
                        int physicalNumerator = Marshal.ReadInt32(mode, 32), physicalDenominator = Marshal.ReadInt32(mode, 36);
                        if (physicalDenominator <= 0) throw new Exception("Invalid signal refresh denominator");
                        Console.WriteLine("Target signal: {0}/{1} = {2:F6} Hz", physicalNumerator, physicalDenominator, (double)physicalNumerator / physicalDenominator);
                    }
                }
            }
        } finally {
            if (modeData != IntPtr.Zero) Marshal.FreeHGlobal(modeData);
            Marshal.FreeHGlobal(pathData);
        }
    }
}
"@
[ActiveDisplayRates]::Read()
Get-CimInstance Win32_VideoController |
    Select-Object Name, CurrentRefreshRate, CurrentHorizontalResolution, CurrentVerticalResolution |
    ConvertTo-Json -Compress
