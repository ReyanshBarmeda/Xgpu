using System.Text.Json;
using System.Text.Json.Nodes;

namespace Xdos.Host;

internal static class SyncFigma
{
    public static int Run(string root, string[] args)
    {
        if (args.Length == 0)
        {
            Console.Error.WriteLine("Usage: sync-figma path/to/figma-export.json");
            return 2;
        }

        var exportPath = Path.GetFullPath(args[0]);
        if (!File.Exists(exportPath))
        {
            Console.Error.WriteLine($"Not found: {exportPath}");
            return 1;
        }

        var tokensPath = Path.Combine(root, "userland", "design", "tokens.json");
        var tokens = JsonNode.Parse(File.ReadAllText(tokensPath))!.AsObject();
        var export = JsonNode.Parse(File.ReadAllText(exportPath));

        var merged = MergeFigmaIntoTokens(tokens, export);
        File.WriteAllText(tokensPath, merged.ToJsonString(new JsonSerializerOptions { WriteIndented = true }));
        Console.WriteLine($"[xdos] merged Figma export → {Path.GetRelativePath(root, tokensPath)}");
        Codegen.Run(root);
        return 0;
    }

    /// <summary>
    /// Accepts either our tokens shape or a flat map of "color/fg/default" → { "hex": "#..." }.
    /// </summary>
    static JsonObject MergeFigmaIntoTokens(JsonObject tokens, JsonNode? export)
    {
        if (export is null)
            return tokens;

        if (export is JsonObject flat && flat.Any(kv => kv.Key.Contains('/')))
        {
            foreach (var (key, value) in flat)
            {
                if (value is not JsonObject obj)
                    continue;
                SetTokenPath(tokens, key, obj);
            }
            return tokens;
        }

        // Whole-file replace of color/layout sections when export mirrors our schema.
        if (export is JsonObject doc)
        {
            if (doc["color"] is JsonObject color)
                tokens["color"] = color.DeepClone();
            if (doc["layout"] is JsonObject layout)
                tokens["layout"] = layout.DeepClone();
            if (doc["component"] is JsonObject component)
                tokens["component"] = component.DeepClone();
        }

        return tokens;
    }

    static void SetTokenPath(JsonObject root, string path, JsonObject leaf)
    {
        var parts = path.Split('/', StringSplitOptions.RemoveEmptyEntries);
        JsonObject current = root;
        for (var i = 0; i < parts.Length - 1; i++)
        {
            if (current[parts[i]] is not JsonObject next)
            {
                next = new JsonObject();
                current[parts[i]] = next;
            }
            current = next;
        }
        current[parts[^1]] = leaf.DeepClone();
    }
}
