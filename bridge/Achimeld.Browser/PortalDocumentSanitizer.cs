namespace Achimeld.Browser;

public static class PortalDocumentSanitizer
{
    // Clone only for parsing; never alter Steam's live document. Session-bearing scripts,
    // hidden/password values, event code and sensitive attributes stay in the worker.
    public const string Script = """
        () => {
            const root = document.documentElement.cloneNode(true);
            root.querySelectorAll('script,iframe,object,embed,meta').forEach(e => e.remove());
            const sensitive = name => /password|passwd|cookie|session|token|csrf|xsrf|authorization|steamguard|steamlogin|apikey/i.test(name.replace(/[_-]/g, ''));
            root.querySelectorAll('*').forEach(element => {
                const secretInput = element.tagName === 'INPUT' && (['password', 'hidden'].includes((element.getAttribute('type') || '').toLowerCase()) || sensitive(element.getAttribute('name') || ''));
                [...element.attributes].forEach(attribute => {
                    if (/^on/i.test(attribute.name) || sensitive(attribute.name) || (secretInput && attribute.name.toLowerCase() === 'value')) {
                        element.removeAttribute(attribute.name);
                    } else if (['href', 'src', 'action'].includes(attribute.name)) {
                        try {
                            const uri = new URL(attribute.value, document.baseURI);
                            uri.username = ''; uri.password = ''; uri.hash = '';
                            [...uri.searchParams.keys()].forEach(key => { if (sensitive(key) || ['auth','key','login'].includes(key.toLowerCase())) uri.searchParams.set(key, '[REDACTED]'); });
                            element.setAttribute(attribute.name, uri.href);
                        } catch { element.removeAttribute(attribute.name); }
                    }
                });
            });
            return root.outerHTML;
        }
        """;

    public static ManagedPageDocument ForTransport(ManagedPageDocument page)
    {
        RedactionService redaction = new();
        return new ManagedPageDocument(new Uri(RedactionService.RedactUri(page.Uri)), redaction.Redact(page.Title), redaction.Redact(page.Html));
    }
}
