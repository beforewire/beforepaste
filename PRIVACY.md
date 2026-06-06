# Privacy

BeforePaste is designed as a local clipboard protection tool.

## What Stays Local

BeforePaste does not upload clipboard contents, prompts, redaction output,
detected secrets, browser URLs, terminal commands, file paths, or target app
names to a cloud service.

Redaction runs on the user's device. Runtime state used for target detection is
stored locally under the user's BeforePaste config directory.

## Local Activity Counts

BeforePaste stores a local append-only counter for protected paste activity.
The counter records timestamp and count only. It does not store clipboard text,
pattern names, target names, or redacted values.

The tray's `Protected 24h` value is computed from this local counter.

## Telemetry

BeforePaste does not include default in-app usage telemetry in the public source
release. It does not send app-open events, target detection events, or redaction
events to BeforeWire.

Website analytics, if used for the public website, are separate from the app and
should be documented on the website.

## Updates

CLI update checks query GitHub Releases to see whether a newer version exists.
Desktop auto-update endpoints are not wired in the current public source
release.

## Reports

Please do not send real secrets or private clipboard contents in bug reports.
Use synthetic examples when reporting missed detections or false positives.
