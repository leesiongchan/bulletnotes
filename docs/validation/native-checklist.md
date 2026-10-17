# Interactive native validation

Run `scripts/native-preview.sh` on this Mac. It builds with Rust 1.99.1 and opens an ad-hoc signed development bundle. It is not notarized or production-ready. The preview has a separate bundle identifier and no storage or network client. All typed text is temporary and disappears on Quit.

Use throwaway text only. The **BN test** menu-bar entry can reopen or quit the preview. Option+A is registered only for the preview lifetime; if another app owns it, the footer reports the conflict and the menu still works. Quit releases the registration. Normal Dock access is retained.

Check these on your real desktop; automated event injection does not establish IME or VoiceOver correctness:

1. Type, select across lines and emoji, replace selections, and use Command+Z / Shift+Command+Z. Paste indented multiline throwaway text. Check the source stays correct.
2. Use your actual Japanese/Chinese or other input method, including composition, candidate selection, and cancellation. If no suitable input method is available, report **untested**.
3. With VoiceOver, check the note can be found, read and edited, and the Pin, Hide, Quit and menu actions have useful names. If VoiceOver was not checked, report **untested**.
4. Resize down to the minimum, type enough lines to scroll, and drag using the top strip. Check the caret stays visible and body selection does not drag the window.
5. Toggle Pin and switch to another ordinary window; unpin and repeat. Option+A should show the preview, focus its editor, and hide it on the next press. Hiding should return focus to the preceding app and reopening should retain text and selection.
6. Repeat show/hide from a full-screen app/another Space. Record what happens rather than assuming ordinary-window behavior carries over.

Report which checks passed, failed, or remain untested. Include the OS version and any unexpected focus/caret behavior. Do not paste personal note contents or clipboard data into the report.

The failed native-blur experiment and inline Markdown colors are deferred by user-approved revisions B and A. They are not part of this solid-paper preview's acceptance.
