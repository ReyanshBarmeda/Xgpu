namespace Xdos.Host;

internal static class NewApp
{
    public static int Run(string root, string[] args)
    {
        if (args.Length == 0)
        {
            Console.Error.WriteLine("Usage: new-app AppName");
            return 2;
        }

        var name = args[0];
        var appDir = Path.Combine(root, "userland", "apps", name);
        if (Directory.Exists(appDir))
        {
            Console.Error.WriteLine($"Already exists: {appDir}");
            return 1;
        }

        Directory.CreateDirectory(appDir);
        File.Copy(
            Path.Combine(root, "userland", "apps", "_template", "App.csproj"),
            Path.Combine(appDir, $"{name}.csproj"),
            overwrite: false);
        File.WriteAllText(
            Path.Combine(appDir, "Program.cs"),
            File.ReadAllText(Path.Combine(root, "userland", "apps", "_template", "Program.cs"))
                .Replace("_AppName_", name));

        Console.WriteLine($"[xdos] scaffolded userland/apps/{name}");
        Console.WriteLine("  dotnet publish -c Release  (after ring-3 loader is wired for your target)");
        return 0;
    }
}
