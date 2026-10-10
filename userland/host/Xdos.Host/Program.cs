using Xdos.Host;

return Cli.Run(args);

internal static class Cli
{
    public static int Run(string[] args)
    {
        if (args.Length == 0)
        {
            PrintHelp();
            return 0;
        }

        var root = RepoRoot();
        return args[0] switch
        {
            "codegen" => Codegen.Run(root),
            "new-app" => NewApp.Run(root, args.Skip(1).ToArray()),
            "sync-figma" => SyncFigma.Run(root, args.Skip(1).ToArray()),
            _ => Unknown(args[0]),
        };
    }

    static string RepoRoot()
    {
        var dir = new DirectoryInfo(AppContext.BaseDirectory);
        while (dir is not null)
        {
            if (File.Exists(Path.Combine(dir.FullName, "Cargo.toml")))
                return dir.FullName;
            dir = dir.Parent;
        }
        throw new InvalidOperationException("Could not find repo root (Cargo.toml). Run from the xdos tree.");
    }

    static int Unknown(string cmd)
    {
        Console.Error.WriteLine($"Unknown command: {cmd}");
        PrintHelp();
        return 2;
    }

    static void PrintHelp()
    {
        Console.WriteLine("""
            Xdos host tools (Windows / .NET 8)

              dotnet run --project userland/host/Xdos.Host -- codegen
                  Regenerate syscall + theme C# from JSON contracts.

              dotnet run --project userland/host/Xdos.Host -- new-app MyApp
                  Scaffold userland/apps/MyApp (C# Native AOT template + ABI copy).

              dotnet run --project userland/host/Xdos.Host -- sync-figma export.json
                  Merge Figma variable export into userland/design/tokens.json.
            """);
    }
}
