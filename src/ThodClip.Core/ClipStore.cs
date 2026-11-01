using System.Globalization;
using System.Text;
using System.Text.Json;
using Microsoft.Data.Sqlite;

namespace ThodClip.Core;

/// <summary>SQLite-backed store for clipboard entries and archived screenshots.</summary>
public sealed class ClipStore : IDisposable
{
    private const string Schema = """
        CREATE TABLE IF NOT EXISTS clips(
          id INTEGER PRIMARY KEY AUTOINCREMENT,
          kind TEXT NOT NULL,
          content TEXT NOT NULL DEFAULT '',
          search TEXT NOT NULL DEFAULT '',
          files TEXT NOT NULL DEFAULT '[]',
          blob TEXT NULL,
          width INTEGER NOT NULL DEFAULT 0,
          height INTEGER NOT NULL DEFAULT 0,
          bytes INTEGER NOT NULL DEFAULT 0,
          hash TEXT NOT NULL,
          source TEXT NULL,
          created_ms INTEGER NOT NULL,
          updated_ms INTEGER NOT NULL,
          repeat_count INTEGER NOT NULL DEFAULT 1,
          pinned INTEGER NOT NULL DEFAULT 0,
          truncated INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS ix_clips_created ON clips(created_ms DESC);
        CREATE INDEX IF NOT EXISTS ix_clips_hash ON clips(hash);
        CREATE TABLE IF NOT EXISTS shots(
          id TEXT PRIMARY KEY,
          relative TEXT NOT NULL,
          thumb TEXT NOT NULL,
          width INTEGER NOT NULL DEFAULT 0,
          height INTEGER NOT NULL DEFAULT 0,
          bytes INTEGER NOT NULL DEFAULT 0,
          hash TEXT NOT NULL,
          source TEXT NOT NULL DEFAULT '',
          name TEXT NOT NULL DEFAULT '',
          search TEXT NOT NULL DEFAULT '',
          created_ms INTEGER NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS ix_shots_hash ON shots(hash);
        CREATE INDEX IF NOT EXISTS ix_shots_created ON shots(created_ms DESC);
        """;

    private readonly object _sync = new();
    private readonly SqliteConnection _connection;
    private readonly ClipSettings _settings;
    private long _revision;

    public string Root { get; }
    public string BlobDirectory { get; }
    public string ThumbDirectory { get; }
    public string DatabasePath { get; }
    public long Revision => Interlocked.Read(ref _revision);

    public ClipStore(string root, ClipSettings settings)
    {
        Root = Path.GetFullPath(root);
        BlobDirectory = Path.Combine(Root, "blobs");
        ThumbDirectory = Path.Combine(Root, "thumbs");
        DatabasePath = Path.Combine(Root, "clip.db");
        Directory.CreateDirectory(Root);
        Directory.CreateDirectory(BlobDirectory);
        Directory.CreateDirectory(ThumbDirectory);
        Directory.CreateDirectory(Path.Combine(Root, "shots", "orig"));
        Directory.CreateDirectory(Path.Combine(Root, "shots", "thumb"));
        _settings = settings;
        _connection = new SqliteConnection(new SqliteConnectionStringBuilder
        {
            DataSource = DatabasePath,
            Mode = SqliteOpenMode.ReadWriteCreate,
            Cache = SqliteCacheMode.Shared
        }.ToString());
        _connection.Open();
        Exec("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL; PRAGMA busy_timeout=5000;");
        Exec(Schema);
        _revision = 1;
    }

    private void Exec(string sql)
    {
        using var command = _connection.CreateCommand();
        command.CommandText = sql;
        command.ExecuteNonQuery();
    }

    private static long Now() => DateTimeOffset.UtcNow.ToUnixTimeMilliseconds();

    private static string Iso(long milliseconds) =>
        DateTimeOffset.FromUnixTimeMilliseconds(milliseconds).ToString("O", CultureInfo.InvariantCulture);

    private static string Lower(string? value) => (value ?? "").ToLowerInvariant();

    private static string Preview(string text)
    {
        var builder = new StringBuilder(Math.Min(text.Length, 720));
        var previousSpace = false;
        foreach (var ch in text)
        {
            if (ch is '\r' or '\n' or '\t')
            {
                if (!previousSpace) { builder.Append(' '); previousSpace = true; }
                continue;
            }
            if (char.IsWhiteSpace(ch))
            {
                if (!previousSpace) { builder.Append(' '); previousSpace = true; }
                continue;
            }
            builder.Append(ch);
            previousSpace = false;
            if (builder.Length >= 700) { builder.Append('…'); break; }
        }
        return builder.ToString().Trim();
    }

    private static string Resolve(string root, string relative)
    {
        var path = Path.GetFullPath(Path.Combine(root, relative.Replace('/', Path.DirectorySeparatorChar)));
        var fullRoot = Path.GetFullPath(root).TrimEnd(Path.DirectorySeparatorChar) + Path.DirectorySeparatorChar;
        if (!path.StartsWith(fullRoot, StringComparison.OrdinalIgnoreCase))
            throw new InvalidDataException("Путь выходит за пределы каталога данных.");
        return path;
    }

    public string ResolveBlob(ClipItem item) => string.IsNullOrWhiteSpace(item.Blob)
        ? throw new FileNotFoundException("Изображение не найдено.")
        : Resolve(Root, item.Blob);

    public string ResolveShot(ShotItem item) => Resolve(Root, item.RelativePath);
    public string ResolveShotThumb(ShotItem item) => Resolve(Root, item.ThumbRelativePath);

    private void Bump() => Interlocked.Increment(ref _revision);

    private void DeleteFiles(params string?[] relatives)
    {
        foreach (var relative in relatives)
        {
            if (string.IsNullOrWhiteSpace(relative)) continue;
            try
            {
                var path = Resolve(Root, relative);
                if (File.Exists(path)) File.Delete(path);
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException) { }
        }
    }

    private static string StoreBlob(string directory, string hash, string extension, byte[] data)
    {
        Directory.CreateDirectory(directory);
        var path = Path.Combine(directory, hash + extension);
        if (File.Exists(path)) return path;
        var temporary = path + "." + Guid.NewGuid().ToString("N") + ".tmp";
        try
        {
            File.WriteAllBytes(temporary, data);
            File.Move(temporary, path, true);
            return path;
        }
        finally
        {
            if (File.Exists(temporary)) File.Delete(temporary);
        }
    }

    private long InsertClip(string kind, string content, string search, string filesJson, string? blob,
        int width, int height, long bytes, string hash, string? source, bool truncated)
    {
        var now = Now();
        long id;
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            if (_settings.DedupeSeconds > 0)
            {
                using var find = _connection.CreateCommand();
                find.Transaction = transaction;
                find.CommandText = "SELECT id, updated_ms FROM clips WHERE hash=$hash ORDER BY id DESC LIMIT 1";
                find.Parameters.AddWithValue("$hash", hash);
                using var reader = find.ExecuteReader();
                if (reader.Read())
                {
                    var existing = reader.GetInt64(0);
                    var updated = reader.GetInt64(1);
                    reader.Close();
                    if (Now() - updated <= _settings.DedupeSeconds * 1000L)
                    {
                        using var update = _connection.CreateCommand();
                        update.Transaction = transaction;
                        update.CommandText = "UPDATE clips SET updated_ms=$now, repeat_count=repeat_count+1, source=COALESCE($source,source) WHERE id=$id";
                        update.Parameters.AddWithValue("$now", now);
                        update.Parameters.AddWithValue("$source", (object?)source ?? DBNull.Value);
                        update.Parameters.AddWithValue("$id", existing);
                        update.ExecuteNonQuery();
                        transaction.Commit();
                        Bump();
                        return existing;
                    }
                }
            }
            using var insert = _connection.CreateCommand();
            insert.Transaction = transaction;
            insert.CommandText = """
                INSERT INTO clips(kind,content,search,files,blob,width,height,bytes,hash,source,created_ms,updated_ms,repeat_count,pinned,truncated)
                VALUES($kind,$content,$search,$files,$blob,$width,$height,$bytes,$hash,$source,$now,$now,1,0,$truncated);
                SELECT last_insert_rowid();
                """;
            insert.Parameters.AddWithValue("$kind", kind);
            insert.Parameters.AddWithValue("$content", content);
            insert.Parameters.AddWithValue("$search", search);
            insert.Parameters.AddWithValue("$files", filesJson);
            insert.Parameters.AddWithValue("$blob", (object?)blob ?? DBNull.Value);
            insert.Parameters.AddWithValue("$width", width);
            insert.Parameters.AddWithValue("$height", height);
            insert.Parameters.AddWithValue("$bytes", bytes);
            insert.Parameters.AddWithValue("$hash", hash);
            insert.Parameters.AddWithValue("$source", (object?)source ?? DBNull.Value);
            insert.Parameters.AddWithValue("$now", now);
            insert.Parameters.AddWithValue("$truncated", truncated ? 1 : 0);
            id = Convert.ToInt64(insert.ExecuteScalar(), CultureInfo.InvariantCulture);
            transaction.Commit();
        }
        PruneClips();
        Bump();
        return id;
    }

    public long AddText(string text, string? source)
    {
        if (string.IsNullOrEmpty(text)) return 0;
        var truncated = false;
        var encoded = Encoding.UTF8.GetBytes(text);
        if (encoded.Length > _settings.MaxTextBytes)
        {
            var take = _settings.MaxTextBytes;
            while (take > 0 && (encoded[take] & 0xC0) == 0x80) take--;
            text = Encoding.UTF8.GetString(encoded, 0, take);
            truncated = true;
        }
        var hash = ImageFiles.HashString("text\n" + text);
        var search = Lower(text + "\n" + source);
        return InsertClip("text", text, search, "[]", null, 0, 0, Encoding.UTF8.GetByteCount(text), hash, source, truncated);
    }

    public long AddFiles(IReadOnlyList<string> rawFiles, string? source)
    {
        var files = rawFiles
            .Where(x => !string.IsNullOrWhiteSpace(x))
            .Select(x => x.Trim())
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Take(500)
            .ToArray();
        if (files.Length == 0) return 0;
        var content = string.Join(Environment.NewLine, files);
        var hash = ImageFiles.HashString("files\n" + string.Join("\n", files.OrderBy(x => x, StringComparer.OrdinalIgnoreCase)));
        var bytes = files.Sum(x => (long)Encoding.UTF8.GetByteCount(x));
        var search = Lower(content + "\n" + source);
        return InsertClip("files", content, search, JsonSerializer.Serialize(files), null, 0, 0, bytes, hash, source, false);
    }

    public long AddImage(byte[] png, int width, int height, string? source, IReadOnlyList<string>? files = null)
    {
        if (png.Length == 0 || width <= 0 || height <= 0) return 0;
        var hash = ImageFiles.HashBytes(png);
        var blob = StoreBlob(BlobDirectory, hash, ".png", png);
        var thumbPath = Path.Combine(ThumbDirectory, hash + ".jpg");
        if (!File.Exists(thumbPath))
        {
            try
            {
                using var stream = new MemoryStream(png);
                using var image = System.Drawing.Image.FromStream(stream);
                var thumb = ImageFiles.Thumbnail(image);
                StoreBlob(ThumbDirectory, hash, ".jpg", thumb);
            }
            catch (Exception ex) when (ex is ArgumentException or OutOfMemoryException or IOException) { }
        }
        var originals = (files ?? Array.Empty<string>())
            .Where(x => !string.IsNullOrWhiteSpace(x))
            .Select(x => x.Trim())
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .Take(20)
            .ToArray();
        var relative = "blobs/" + Path.GetFileName(blob);
        var search = Lower("изображение " + width + "×" + height + "\n" + source + "\n" + string.Join("\n", originals));
        return InsertClip("image", "", search, JsonSerializer.Serialize(originals), relative, width, height,
            png.Length, hash, source, false);
    }

    public string? ThumbnailFor(ClipItem item)
    {
        var path = Path.Combine(ThumbDirectory, Path.GetFileNameWithoutExtension(item.Blob ?? "") + ".jpg");
        return File.Exists(path) ? path : null;
    }

    /// <summary>
    /// Older versions stored a copied picture as a "files" entry when the clipboard delivered a
    /// temporary path such as {GUID}.png. Those entries get a real preview once the file still exists.
    /// </summary>
    public int UpgradeImageFileClips(int limit = 120)
    {
        var candidates = new List<(long Id, string Path)>();
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = "SELECT id,files FROM clips WHERE kind='files' AND blob IS NULL ORDER BY id DESC LIMIT $limit";
            command.Parameters.AddWithValue("$limit", Math.Clamp(limit, 1, 2000));
            using var reader = command.ExecuteReader();
            while (reader.Read())
            {
                string[]? files = null;
                try { files = JsonSerializer.Deserialize<string[]>(reader.GetString(1)); }
                catch (JsonException) { }
                if (files is not { Length: 1 } || string.IsNullOrWhiteSpace(files[0])) continue;
                candidates.Add((reader.GetInt64(0), files[0]));
            }
        }

        var converted = 0;
        foreach (var (id, path) in candidates)
        {
            if (!ImageFiles.IsImagePath(path) || !File.Exists(path)) continue;
            try
            {
                var png = ImageFiles.TryEncodeFileAsPng(path, out var width, out var height);
                if (png is null) continue;
                var hash = ImageFiles.HashBytes(png);
                var blob = StoreBlob(BlobDirectory, hash, ".png", png);
                var thumbPath = Path.Combine(ThumbDirectory, hash + ".jpg");
                if (!File.Exists(thumbPath))
                {
                    using var stream = new MemoryStream(png);
                    using var image = System.Drawing.Image.FromStream(stream);
                    StoreBlob(ThumbDirectory, hash, ".jpg", ImageFiles.Thumbnail(image));
                }
                lock (_sync)
                {
                    using var update = _connection.CreateCommand();
                    update.CommandText = """
                        UPDATE clips SET kind='image', content='', search=$search, blob=$blob,
                          width=$width, height=$height, bytes=$bytes, hash=$hash
                        WHERE id=$id AND kind='files'
                        """;
                    update.Parameters.AddWithValue("$search", Lower("изображение " + width + "×" + height + "\n" + path));
                    update.Parameters.AddWithValue("$blob", "blobs/" + Path.GetFileName(blob));
                    update.Parameters.AddWithValue("$width", width);
                    update.Parameters.AddWithValue("$height", height);
                    update.Parameters.AddWithValue("$bytes", png.Length);
                    update.Parameters.AddWithValue("$hash", hash);
                    update.Parameters.AddWithValue("$id", id);
                    if (update.ExecuteNonQuery() > 0) converted++;
                }
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or OutOfMemoryException or InvalidDataException) { }
        }
        if (converted > 0) Bump();
        return converted;
    }

    public ClipPage QueryClips(string? query, string? kind, bool pinnedOnly, string? date, int offset, int limit)
    {
        offset = Math.Max(0, offset);
        limit = Math.Clamp(limit, 1, 200);
        const int listTextLimit = 4000;
        var where = new List<string>();
        if (kind is "text" or "image" or "files") where.Add("kind=$kind");
        if (pinnedOnly) where.Add("pinned=1");
        if (!string.IsNullOrWhiteSpace(query)) where.Add("instr(search,$q)>0");
        long? from = null, to = null;
        if (!string.IsNullOrWhiteSpace(date) && DateOnly.TryParseExact(date, "yyyy-MM-dd", CultureInfo.InvariantCulture,
                DateTimeStyles.None, out var day))
        {
            from = LocalDayStart(day);
            to = LocalDayStart(day.AddDays(1));
            where.Add("created_ms>=$from AND created_ms<$to");
        }
        var clause = where.Count == 0 ? "" : " WHERE " + string.Join(" AND ", where);
        lock (_sync)
        {
            using var count = _connection.CreateCommand();
            count.CommandText = "SELECT COUNT(*) FROM clips" + clause;
            if (kind is "text" or "image" or "files") count.Parameters.AddWithValue("$kind", kind);
            if (!string.IsNullOrWhiteSpace(query)) count.Parameters.AddWithValue("$q", query.Trim().ToLowerInvariant());
            if (from is not null)
            {
                count.Parameters.AddWithValue("$from", from.Value);
                count.Parameters.AddWithValue("$to", to!.Value);
            }
            var total = Convert.ToInt32(count.ExecuteScalar(), CultureInfo.InvariantCulture);

            using var command = _connection.CreateCommand();
            command.CommandText = """
                SELECT id,kind,content,files,blob,width,height,bytes,source,created_ms,updated_ms,repeat_count,pinned,truncated
                FROM clips
                """ + clause + " ORDER BY pinned DESC, created_ms DESC, id DESC LIMIT $limit OFFSET $offset";
            if (kind is "text" or "image" or "files") command.Parameters.AddWithValue("$kind", kind);
            if (!string.IsNullOrWhiteSpace(query)) command.Parameters.AddWithValue("$q", query.Trim().ToLowerInvariant());
            if (from is not null)
            {
                command.Parameters.AddWithValue("$from", from.Value);
                command.Parameters.AddWithValue("$to", to!.Value);
            }
            command.Parameters.AddWithValue("$limit", limit);
            command.Parameters.AddWithValue("$offset", offset);
            var items = new List<ClipItem>();
            using var reader = command.ExecuteReader();
            while (reader.Read()) items.Add(MapClip(reader, true, listTextLimit));
            return new ClipPage { Revision = Revision, Total = total, Offset = offset, Limit = limit, Items = items, Date = date };
        }
    }

    private static long LocalDayStart(DateOnly day)
    {
        var local = day.ToDateTime(TimeOnly.MinValue, DateTimeKind.Unspecified);
        return new DateTimeOffset(local, TimeZoneInfo.Local.GetUtcOffset(local)).ToUnixTimeMilliseconds();
    }

    /// <summary>Per-day clip counts for one calendar month, in local time.</summary>
    public ClipCalendar QueryCalendar(string month)
    {
        if (!DateOnly.TryParseExact(month + "-01", "yyyy-MM-dd", CultureInfo.InvariantCulture,
                DateTimeStyles.None, out var first))
            throw new ArgumentException("Месяц должен быть в формате ГГГГ-ММ.", nameof(month));

        var next = first.AddMonths(1);
        var counts = new Dictionary<string, int>(StringComparer.Ordinal);
        lock (_sync)
        {
            using (var command = _connection.CreateCommand())
            {
                command.CommandText = "SELECT created_ms FROM clips WHERE created_ms>=$from AND created_ms<$to";
                command.Parameters.AddWithValue("$from", LocalDayStart(first));
                command.Parameters.AddWithValue("$to", LocalDayStart(next));
                using var reader = command.ExecuteReader();
                while (reader.Read())
                {
                    var date = DateTimeOffset.FromUnixTimeMilliseconds(reader.GetInt64(0)).ToLocalTime()
                        .ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
                    counts[date] = counts.TryGetValue(date, out var count) ? count + 1 : 1;
                }
            }

            string? firstDate = null, lastDate = null;
            using (var bounds = _connection.CreateCommand())
            {
                bounds.CommandText = "SELECT MIN(created_ms), MAX(created_ms) FROM clips";
                using var reader = bounds.ExecuteReader();
                if (reader.Read() && !reader.IsDBNull(0))
                {
                    firstDate = DateTimeOffset.FromUnixTimeMilliseconds(reader.GetInt64(0)).ToLocalTime()
                        .ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
                    lastDate = DateTimeOffset.FromUnixTimeMilliseconds(reader.GetInt64(1)).ToLocalTime()
                        .ToString("yyyy-MM-dd", CultureInfo.InvariantCulture);
                }
            }

            return new ClipCalendar
            {
                Month = month,
                Total = counts.Values.Sum(),
                FirstDate = firstDate,
                LastDate = lastDate,
                Days = counts
                    .OrderBy(pair => pair.Key, StringComparer.Ordinal)
                    .Select(pair => new CalendarDay { Date = pair.Key, Count = pair.Value })
                    .ToArray()
            };
        }
    }

    private static ClipItem MapClip(SqliteDataReader reader, bool includeText, int textLimit = int.MaxValue)
    {
        var content = reader.GetString(2);
        var clipped = includeText && content.Length > textLimit;
        return new ClipItem
        {
            Id = reader.GetInt64(0),
            Kind = reader.GetString(1),
            Text = includeText ? (clipped ? content[..textLimit] : content) : "",
            TextClipped = clipped,
            Preview = Preview(content),
            Files = JsonSerializer.Deserialize<string[]>(reader.GetString(3)) ?? Array.Empty<string>(),
            Blob = reader.IsDBNull(4) ? null : reader.GetString(4),
            Width = reader.GetInt32(5),
            Height = reader.GetInt32(6),
            Bytes = reader.GetInt64(7),
            Source = reader.IsDBNull(8) ? null : reader.GetString(8),
            CreatedAt = Iso(reader.GetInt64(9)),
            UpdatedAt = Iso(reader.GetInt64(10)),
            RepeatCount = reader.GetInt32(11),
            Pinned = reader.GetInt32(12) != 0,
            Truncated = reader.GetInt32(13) != 0
        };
    }

    public ClipItem? GetClip(long id)
    {
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = """
                SELECT id,kind,content,files,blob,width,height,bytes,source,created_ms,updated_ms,repeat_count,pinned,truncated
                FROM clips WHERE id=$id
                """;
            command.Parameters.AddWithValue("$id", id);
            using var reader = command.ExecuteReader();
            return reader.Read() ? MapClip(reader, includeText: true) : null;
        }
    }

    public bool PinClip(long id, bool pinned)
    {
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = "UPDATE clips SET pinned=$pinned WHERE id=$id";
            command.Parameters.AddWithValue("$pinned", pinned ? 1 : 0);
            command.Parameters.AddWithValue("$id", id);
            var changed = command.ExecuteNonQuery() > 0;
            if (changed) Bump();
            return changed;
        }
    }

    private void DeleteUnusedBlobs(IEnumerable<string> blobs)
    {
        foreach (var blob in blobs.Where(x => !string.IsNullOrWhiteSpace(x)).Distinct(StringComparer.OrdinalIgnoreCase))
        {
            bool used;
            lock (_sync)
            {
                using var check = _connection.CreateCommand();
                check.CommandText = "SELECT 1 FROM clips WHERE blob=$blob LIMIT 1";
                check.Parameters.AddWithValue("$blob", blob);
                used = check.ExecuteScalar() is not null;
            }
            if (used) continue;
            var name = Path.GetFileNameWithoutExtension(blob);
            DeleteFiles(blob, "thumbs/" + name + ".jpg");
        }
    }

    public int DeleteClips(IEnumerable<long> ids)
    {
        var list = ids.Distinct().Take(1000).ToArray();
        if (list.Length == 0) return 0;
        var removed = new List<string>();
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            foreach (var id in list)
            {
                using var select = _connection.CreateCommand();
                select.Transaction = transaction;
                select.CommandText = "SELECT blob FROM clips WHERE id=$id";
                select.Parameters.AddWithValue("$id", id);
                if (select.ExecuteScalar() is string blob) removed.Add(blob);
                using var delete = _connection.CreateCommand();
                delete.Transaction = transaction;
                delete.CommandText = "DELETE FROM clips WHERE id=$id";
                delete.Parameters.AddWithValue("$id", id);
                delete.ExecuteNonQuery();
            }
            transaction.Commit();
        }
        DeleteUnusedBlobs(removed);
        Bump();
        return list.Length;
    }

    public int ClearClips(bool keepPinned)
    {
        var removed = new List<string>();
        int count;
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            using (var select = _connection.CreateCommand())
            {
                select.Transaction = transaction;
                select.CommandText = keepPinned ? "SELECT blob FROM clips WHERE pinned=0" : "SELECT blob FROM clips";
                using var reader = select.ExecuteReader();
                while (reader.Read()) if (!reader.IsDBNull(0)) removed.Add(reader.GetString(0));
            }
            using var delete = _connection.CreateCommand();
            delete.Transaction = transaction;
            delete.CommandText = keepPinned ? "DELETE FROM clips WHERE pinned=0" : "DELETE FROM clips";
            count = delete.ExecuteNonQuery();
            transaction.Commit();
        }
        DeleteUnusedBlobs(removed);
        Bump();
        return count;
    }

    /// <summary>Applies retention limits after settings change.</summary>
    public void PruneNow()
    {
        PruneClips();
        PruneShots();
        Bump();
    }

    private void PruneClips()
    {
        if (_settings.MaxClips <= 0) return;
        var removed = new List<string>();
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            using (var select = _connection.CreateCommand())
            {
                select.Transaction = transaction;
                select.CommandText = """
                    SELECT blob FROM clips
                    WHERE pinned=0 AND id NOT IN (
                      SELECT id FROM clips ORDER BY pinned DESC, created_ms DESC, id DESC LIMIT $max
                    )
                    """;
                select.Parameters.AddWithValue("$max", _settings.MaxClips);
                using var reader = select.ExecuteReader();
                while (reader.Read()) if (!reader.IsDBNull(0)) removed.Add(reader.GetString(0));
            }
            using var delete = _connection.CreateCommand();
            delete.Transaction = transaction;
            delete.CommandText = """
                DELETE FROM clips
                WHERE pinned=0 AND id NOT IN (
                  SELECT id FROM clips ORDER BY pinned DESC, created_ms DESC, id DESC LIMIT $max
                )
                """;
            delete.Parameters.AddWithValue("$max", _settings.MaxClips);
            delete.ExecuteNonQuery();
            transaction.Commit();
        }
        DeleteUnusedBlobs(removed);
    }

    // ---------------------------------------------------------------- screenshots

    private const int DuplicateWindowSeconds = 900;
    private const double DuplicateDistance = 4.0;

    /// <summary>
    /// Finds a recently archived screenshot with the same size and a nearly identical picture.
    /// Windows can deliver one screenshot both through the clipboard and as a saved file,
    /// and the two copies rarely have identical bytes.
    /// </summary>
    public string? FindSimilarShot(int width, int height, byte[] preview, int windowSeconds = DuplicateWindowSeconds)
    {
        if (width <= 0 || height <= 0 || preview.Length == 0) return null;
        var fingerprint = ImageFiles.Fingerprint(preview);
        if (fingerprint is null) return null;
        var since = Now() - Math.Max(1, windowSeconds) * 1000L;
        var candidates = new List<(string Id, string Thumb)>();
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = """
                SELECT id,thumb FROM shots
                WHERE width=$width AND height=$height AND created_ms>=$since
                ORDER BY created_ms DESC LIMIT 12
                """;
            command.Parameters.AddWithValue("$width", width);
            command.Parameters.AddWithValue("$height", height);
            command.Parameters.AddWithValue("$since", since);
            using var reader = command.ExecuteReader();
            while (reader.Read()) candidates.Add((reader.GetString(0), reader.GetString(1)));
        }
        foreach (var (id, thumb) in candidates)
        {
            try
            {
                var path = Resolve(Root, thumb);
                if (!File.Exists(path)) continue;
                var other = ImageFiles.Fingerprint(File.ReadAllBytes(path));
                if (other is null) continue;
                if (ImageFiles.Distance(fingerprint, other) <= DuplicateDistance) return id;
            }
            catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or InvalidDataException or ArgumentException or OutOfMemoryException) { }
        }
        return null;
    }

    public string? AddScreenshotFile(string sourcePath, string? source, bool enforceLimit = true)
    {
        if (!File.Exists(sourcePath)) return null;
        var extension = Path.GetExtension(sourcePath).ToLowerInvariant();
        if (!ImageFiles.Extensions.Contains(extension)) return null;
        if (!ImageFiles.TryReadInfo(sourcePath, out var width, out var height)) return null;
        string hash;
        using (var stream = File.OpenRead(sourcePath))
            hash = Convert.ToHexString(System.Security.Cryptography.SHA256.HashData(stream)).ToLowerInvariant();

        lock (_sync)
        {
            using (var exists = _connection.CreateCommand())
            {
                exists.CommandText = "SELECT id FROM shots WHERE hash=$hash";
                exists.Parameters.AddWithValue("$hash", hash);
                if (exists.ExecuteScalar() is string existing) return existing;
            }
        }

        var thumb = ImageFiles.Thumbnail(sourcePath);
        var similar = FindSimilarShot(width, height, thumb);
        if (similar is not null) return similar;

        var id = DateTime.UtcNow.ToString("yyyyMMdd-HHmmss", CultureInfo.InvariantCulture) + "-" + hash[..8];
        var month = DateTime.UtcNow.ToString("yyyy-MM", CultureInfo.InvariantCulture);
        var relative = $"shots/orig/{month}/{id}{extension}";
        var thumbRelative = $"shots/thumb/{month}/{id}.jpg";
        var target = Resolve(Root, relative);
        var thumbTarget = Resolve(Root, thumbRelative);
        Directory.CreateDirectory(Path.GetDirectoryName(target)!);
        Directory.CreateDirectory(Path.GetDirectoryName(thumbTarget)!);
        try
        {
            File.Copy(sourcePath, target, false);
            File.WriteAllBytes(thumbTarget, thumb);
            return InsertShot(id, relative, thumbRelative, width, height, new FileInfo(target).Length, hash,
                source ?? "Файл", Path.GetFileName(sourcePath), enforceLimit);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or OutOfMemoryException)
        {
            TryDelete(target);
            TryDelete(thumbTarget);
            return null;
        }
    }

    public string? AddScreenshotPng(byte[] png, string? source, string? name = null, bool enforceLimit = true)
    {
        if (png.Length == 0) return null;
        int width, height;
        byte[] thumb;
        try
        {
            using var stream = new MemoryStream(png);
            using var image = System.Drawing.Image.FromStream(stream);
            width = image.Width;
            height = image.Height;
            thumb = ImageFiles.Thumbnail(image);
        }
        catch (Exception ex) when (ex is ArgumentException or OutOfMemoryException)
        {
            return null;
        }
        var hash = ImageFiles.HashBytes(png);
        lock (_sync)
        {
            using var exists = _connection.CreateCommand();
            exists.CommandText = "SELECT id FROM shots WHERE hash=$hash";
            exists.Parameters.AddWithValue("$hash", hash);
            if (exists.ExecuteScalar() is string existing) return existing;
        }
        var similar = FindSimilarShot(width, height, thumb);
        if (similar is not null) return similar;
        var id = DateTime.UtcNow.ToString("yyyyMMdd-HHmmss", CultureInfo.InvariantCulture) + "-" + hash[..8];
        var month = DateTime.UtcNow.ToString("yyyy-MM", CultureInfo.InvariantCulture);
        var relative = $"shots/orig/{month}/{id}.png";
        var thumbRelative = $"shots/thumb/{month}/{id}.jpg";
        var target = Resolve(Root, relative);
        var thumbTarget = Resolve(Root, thumbRelative);
        Directory.CreateDirectory(Path.GetDirectoryName(target)!);
        Directory.CreateDirectory(Path.GetDirectoryName(thumbTarget)!);
        try
        {
            File.WriteAllBytes(target, png);
            File.WriteAllBytes(thumbTarget, thumb);
            return InsertShot(id, relative, thumbRelative, width, height, png.Length, hash,
                source ?? "Буфер обмена", name ?? $"Снимок {width}×{height}", enforceLimit);
        }
        catch (Exception ex) when (ex is IOException or UnauthorizedAccessException or ArgumentException or OutOfMemoryException)
        {
            TryDelete(target);
            TryDelete(thumbTarget);
            return null;
        }
    }

    private string InsertShot(string id, string relative, string thumb, int width, int height, long bytes,
        string hash, string source, string name, bool enforceLimit)
    {
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = """
                INSERT OR IGNORE INTO shots(id,relative,thumb,width,height,bytes,hash,source,name,search,created_ms)
                VALUES($id,$relative,$thumb,$width,$height,$bytes,$hash,$source,$name,$search,$now)
                """;
            command.Parameters.AddWithValue("$id", id);
            command.Parameters.AddWithValue("$relative", relative);
            command.Parameters.AddWithValue("$thumb", thumb);
            command.Parameters.AddWithValue("$width", width);
            command.Parameters.AddWithValue("$height", height);
            command.Parameters.AddWithValue("$bytes", bytes);
            command.Parameters.AddWithValue("$hash", hash);
            command.Parameters.AddWithValue("$source", source);
            command.Parameters.AddWithValue("$name", name);
            command.Parameters.AddWithValue("$search", Lower(name + "\n" + source));
            command.Parameters.AddWithValue("$now", Now());
            command.ExecuteNonQuery();
        }
        if (enforceLimit) PruneShots();
        Bump();
        return id;
    }

    public ShotPage QueryShots(string? query, int offset, int limit)
    {
        offset = Math.Max(0, offset);
        limit = Math.Clamp(limit, 1, 200);
        var clause = string.IsNullOrWhiteSpace(query) ? "" : " WHERE instr(search,$q)>0";
        lock (_sync)
        {
            using var count = _connection.CreateCommand();
            count.CommandText = "SELECT COUNT(*) FROM shots" + clause;
            if (!string.IsNullOrWhiteSpace(query)) count.Parameters.AddWithValue("$q", query.Trim().ToLowerInvariant());
            var total = Convert.ToInt32(count.ExecuteScalar(), CultureInfo.InvariantCulture);
            using var command = _connection.CreateCommand();
            command.CommandText = """
                SELECT id,relative,thumb,width,height,bytes,hash,source,name,created_ms
                FROM shots
                """ + clause + " ORDER BY created_ms DESC, id DESC LIMIT $limit OFFSET $offset";
            if (!string.IsNullOrWhiteSpace(query)) command.Parameters.AddWithValue("$q", query.Trim().ToLowerInvariant());
            command.Parameters.AddWithValue("$limit", limit);
            command.Parameters.AddWithValue("$offset", offset);
            var items = new List<ShotItem>();
            using var reader = command.ExecuteReader();
            while (reader.Read()) items.Add(MapShot(reader));
            return new ShotPage { Revision = Revision, Total = total, Offset = offset, Limit = limit, Items = items };
        }
    }

    private static ShotItem MapShot(SqliteDataReader reader) => new()
    {
        Id = reader.GetString(0),
        RelativePath = reader.GetString(1),
        ThumbRelativePath = reader.GetString(2),
        Width = reader.GetInt32(3),
        Height = reader.GetInt32(4),
        Bytes = reader.GetInt64(5),
        Hash = reader.GetString(6),
        Source = reader.GetString(7),
        Name = reader.GetString(8),
        CreatedAt = Iso(reader.GetInt64(9))
    };

    public ShotItem? GetShot(string id)
    {
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = """
                SELECT id,relative,thumb,width,height,bytes,hash,source,name,created_ms
                FROM shots WHERE id=$id
                """;
            command.Parameters.AddWithValue("$id", id);
            using var reader = command.ExecuteReader();
            return reader.Read() ? MapShot(reader) : null;
        }
    }

    public bool ShotHashExists(string hash)
    {
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = "SELECT 1 FROM shots WHERE hash=$hash LIMIT 1";
            command.Parameters.AddWithValue("$hash", hash);
            return command.ExecuteScalar() is not null;
        }
    }

    public int DeleteShots(IEnumerable<string> ids)
    {
        var list = ids.Distinct().Take(1000).ToArray();
        if (list.Length == 0) return 0;
        var removed = new List<(string, string)>();
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            foreach (var id in list)
            {
                using var select = _connection.CreateCommand();
                select.Transaction = transaction;
                select.CommandText = "SELECT relative,thumb FROM shots WHERE id=$id";
                select.Parameters.AddWithValue("$id", id);
                using var reader = select.ExecuteReader();
                if (reader.Read()) removed.Add((reader.GetString(0), reader.GetString(1)));
                reader.Close();
                using var delete = _connection.CreateCommand();
                delete.Transaction = transaction;
                delete.CommandText = "DELETE FROM shots WHERE id=$id";
                delete.Parameters.AddWithValue("$id", id);
                delete.ExecuteNonQuery();
            }
            transaction.Commit();
        }
        foreach (var (relative, thumb) in removed) DeleteFiles(relative, thumb);
        Bump();
        return removed.Count;
    }

    public int ClearShots()
    {
        var removed = new List<(string, string)>();
        lock (_sync)
        {
            using var transaction = _connection.BeginTransaction();
            using (var select = _connection.CreateCommand())
            {
                select.Transaction = transaction;
                select.CommandText = "SELECT relative,thumb FROM shots";
                using var reader = select.ExecuteReader();
                while (reader.Read()) removed.Add((reader.GetString(0), reader.GetString(1)));
            }
            using var delete = _connection.CreateCommand();
            delete.Transaction = transaction;
            delete.CommandText = "DELETE FROM shots";
            var count = delete.ExecuteNonQuery();
            transaction.Commit();
            foreach (var (relative, thumb) in removed) DeleteFiles(relative, thumb);
            Bump();
            return count;
        }
    }

    private void PruneShots()
    {
        if (_settings.MaxScreenshotBytes <= 0) return;
        var removed = new List<(string, string)>();
        lock (_sync)
        {
            long total;
            using (var sum = _connection.CreateCommand())
            {
                sum.CommandText = "SELECT COALESCE(SUM(bytes),0) FROM shots";
                total = Convert.ToInt64(sum.ExecuteScalar(), CultureInfo.InvariantCulture);
            }
            if (total <= _settings.MaxScreenshotBytes) return;
            using var transaction = _connection.BeginTransaction();
            using var select = _connection.CreateCommand();
            select.Transaction = transaction;
            select.CommandText = "SELECT id,relative,thumb,bytes FROM shots ORDER BY created_ms ASC, id ASC";
            using var reader = select.ExecuteReader();
            var removeIds = new List<string>();
            while (reader.Read() && total > _settings.MaxScreenshotBytes)
            {
                total -= reader.GetInt64(3);
                removeIds.Add(reader.GetString(0));
                removed.Add((reader.GetString(1), reader.GetString(2)));
            }
            reader.Close();
            foreach (var id in removeIds)
            {
                using var delete = _connection.CreateCommand();
                delete.Transaction = transaction;
                delete.CommandText = "DELETE FROM shots WHERE id=$id";
                delete.Parameters.AddWithValue("$id", id);
                delete.ExecuteNonQuery();
            }
            transaction.Commit();
        }
        foreach (var (relative, thumb) in removed) DeleteFiles(relative, thumb);
    }

    public int VerifyShots()
    {
        var missing = new List<string>();
        lock (_sync)
        {
            using var command = _connection.CreateCommand();
            command.CommandText = "SELECT id,relative,thumb FROM shots";
            using var reader = command.ExecuteReader();
            while (reader.Read())
            {
                if (!File.Exists(Resolve(Root, reader.GetString(1))) || !File.Exists(Resolve(Root, reader.GetString(2))))
                    missing.Add(reader.GetString(0));
            }
        }
        return missing.Count == 0 ? 0 : DeleteShots(missing);
    }

    public StorageStats GetStats()
    {
        lock (_sync)
        {
            using var clips = _connection.CreateCommand();
            clips.CommandText = "SELECT COUNT(*), COALESCE(SUM(bytes),0) FROM clips";
            using var reader = clips.ExecuteReader();
            reader.Read();
            var clipCount = reader.GetInt32(0);
            var clipBytes = reader.GetInt64(1);
            reader.Close();
            using var shots = _connection.CreateCommand();
            shots.CommandText = "SELECT COUNT(*), COALESCE(SUM(bytes),0) FROM shots";
            using var shotReader = shots.ExecuteReader();
            shotReader.Read();
            var shotCount = shotReader.GetInt32(0);
            var shotBytes = shotReader.GetInt64(1);
            shotReader.Close();
            long dbBytes = 0;
            foreach (var suffix in new[] { "", "-wal", "-shm" })
            {
                var file = DatabasePath + suffix;
                if (File.Exists(file)) dbBytes += new FileInfo(file).Length;
            }
            return new StorageStats
            {
                ClipCount = clipCount,
                ShotCount = shotCount,
                ClipBytes = clipBytes,
                ShotBytes = shotBytes,
                DatabaseBytes = dbBytes
            };
        }
    }

    private static void TryDelete(string path)
    {
        try { if (File.Exists(path)) File.Delete(path); } catch (Exception ex) when (ex is IOException or UnauthorizedAccessException) { }
    }

    public void Dispose()
    {
        lock (_sync)
        {
            try { Exec("PRAGMA wal_checkpoint(TRUNCATE);"); } catch (SqliteException) { }
            _connection.Dispose();
        }
    }
}
