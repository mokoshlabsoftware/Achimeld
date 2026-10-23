namespace Achimeld.Browser;

public static class FixedReadScripts
{
    // No evaluation command is exposed by the worker. The driver constructs one
    // pinned configuration URL, keeps credentials in Chromium and bounds bytes
    // while streaming, rather than after APIRequest buffered the whole response.
    public const string Configuration = """
        async ({url, maximumBytes, timeoutMs}) => {
            const controller = new AbortController();
            const timer = setTimeout(() => controller.abort(), timeoutMs);
            let reader;
            try {
                const response = await fetch(url, {method:'GET', credentials:'same-origin',
                    redirect:'error', cache:'no-store', signal:controller.signal});
                if (!response.ok || response.url !== url || !response.body) throw new Error('Configuration read rejected.');
                const declared = response.headers.get('content-length');
                if (declared && Number(declared) > maximumBytes) throw new Error('Configuration exceeds its budget.');
                reader = response.body.getReader();
                const chunks = []; let size = 0;
                while (true) {
                    const {value, done} = await reader.read();
                    if (done) break;
                    size += value.byteLength;
                    if (size > maximumBytes) throw new Error('Configuration exceeds its budget.');
                    chunks.push(value);
                }
                const bytes = new Uint8Array(size); let offset = 0;
                for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.byteLength; }
                return new TextDecoder('utf-8', {fatal:true}).decode(bytes);
            } finally {
                clearTimeout(timer);
                if (reader) { try { await reader.cancel(); } catch {} }
                controller.abort();
            }
        }
        """;
}
