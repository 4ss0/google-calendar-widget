# Google Calendar Widget

A lightweight desktop widget for Windows that displays your Google Calendar events directly on the desktop, with month / week / day views that adapt automatically to the window width.

![Windows 11](https://img.shields.io/badge/Windows-11%20build%2022000%2B-blue)
![Rust](https://img.shields.io/badge/Rust-1.75%2B-orange)
![License](https://img.shields.io/badge/License-Apache%202.0-blue)

## ⚠️ Disclaimer

This project was developed with the assistance of Artificial Intelligence.
The code is provided **as-is**, without any warranty of any kind, express or implied.
The author takes **no responsibility** for any damage, data loss, malfunction, or any other consequence arising from the use, misuse, or inability to use this software.

Use at your own risk. Review the source code before running it if you have any concerns.

## Features

- Transparent, borderless, always-on-bottom desktop window (stays visible after Win+D)
- Month / week / day view, selected automatically based on window width
- Light and dark theme
- Event creation, editing, and deletion via Google Calendar API
- Full recurring-event support (Only this / All events / This and following)
- Inline search filter
- Undo banner after delete (non-recurring events)
- Drag & drop to move events by whole days, DST-safe
- Google OAuth 2.0 with PKCE
- Refresh token encrypted with Windows DPAPI
- Tray icon with Show / Hide / Quit menu
- Autostart via the Windows Run registry key
- Adjustable window opacity

## Requirements

- **Windows 11 build 22000 (21H2) or later**
- A Google account
- A Google Cloud project with the Calendar API enabled (see below)

> Windows 11 build 22000 is required because the widget uses `DWMWA_CLOAKED` / `DWMWA_CLOAK` to restore itself after Win+D (Show Desktop). These APIs are only available starting with that build.

---

## Step 1 — Create a Google Cloud project

1. Go to the [Google Cloud Console](https://console.cloud.google.com/).
2. Sign in with your Google account.
3. In the top bar, click the project dropdown and select **New Project**.
4. Give it a name (e.g. `Calendar Widget`) and click **Create**.
5. Wait for the project to be created, then make sure it is selected in the top bar.

---

## Step 2 — Enable the Google Calendar API

1. In the left menu, go to **APIs & Services → Library**.
2. Search for **Google Calendar API**.
3. Click on it and press **Enable**.
4. Wait a few seconds until the API is enabled for your project.

---

## Step 3 — Configure the OAuth consent screen

1. Go to **APIs & Services → OAuth consent screen**.
2. Choose **External** (unless you have a Google Workspace account and prefer **Internal**), then click **Create**.
3. Fill in the required fields:
   - **App name:** `Calendar Widget` (or anything you like)
   - **User support email:** your email
   - **Developer contact information:** your email
4. Click **Save and continue**.
5. On the **Scopes** page, click **Add or remove scopes**, search for:

https://www.googleapis.com/auth/calendar.events

Select it and click **Update**, then **Save and continue**.
6. On the **Test users** page, click **Add users** and add **your own Google account** (the one whose calendar you want to display).
> This is required while the app is in "Testing" mode. Without it, Google will refuse the authorization.
7. Click **Save and continue**, review the summary, and click **Back to dashboard**.

> You do **not** need to publish the app. Leaving it in "Testing" mode is fine for personal use.

---

## Step 4 — Create OAuth 2.0 credentials

1. Go to **APIs & Services → Credentials**.
2. Click **+ Create credentials → OAuth client ID**.
3. For **Application type**, choose **Desktop app**.
4. Give it a name (e.g. `Calendar Widget Desktop`) and click **Create**.
5. A dialog will appear with your **Client ID** and **Client Secret**. Copy both values, or click **Download JSON** to save them.

> The **Client Secret** is required by the widget even though desktop OAuth flows technically allow it to be omitted. Keep both values handy for the next step.

---

## Step 5 — First launch of the widget

1. Download `google-calendar-widget.exe` from the [Releases](../../releases) page and run it.
2. On first launch, a setup screen appears. Fill in:
- **Client ID:** the value from Step 4
- **Client Secret:** the value from Step 4
- **Calendar ID:** leave it as `primary` to use your main calendar, or paste a specific calendar ID (see below)
3. Click **Save and continue**. Your default browser opens the Google consent page.
4. Sign in with the same Google account you added as a test user in Step 3.
5. Accept the requested permissions.
6. The browser shows "Authorization complete". You can close the tab.
7. The widget appears on your desktop with your events.

> The refresh token is stored encrypted with Windows DPAPI under `%APPDATA%\example\gcal-widget\config\`. You will not be asked to authorize again on subsequent launches.

### How to find your Calendar ID

If you want to display a calendar other than your main one:

1. Open [Google Calendar](https://calendar.google.com/) in a browser.
2. In the left sidebar, hover over the calendar you want and click the three-dot menu → **Settings and sharing**.
3. Scroll to the **Integrate calendar** section.
4. Copy the **Calendar ID** (it looks like `xxxxxxxx@group.calendar.google.com`, or your email address for the main calendar).

---

## Usage

- **Navigate:** `<` / `>` buttons change the period; **Today** returns to the current date.
- **Create:** `+` (or click an empty day cell) opens the event form.
- **Edit:** click an event. For recurring instances, choose the scope (*Only this* / *All events* / *This and following*).
- **Move:** drag an event onto another day.
- **Delete:** open the event and press **Delete**. Non-recurring events offer an **Undo** banner for 8 seconds.
- **Search:** use the search field in the top bar to filter events by title.
- **Opacity:** `+` / `-` buttons adjust window transparency.
- **Tray:** right-click the tray icon for Show / Hide / Quit.
- **Autostart:** toggle the `Auto` button in the top bar.

---

## Configuration files

All files live under `%APPDATA%\example\gcal-widget\config\`:

| File | Purpose |
|------|---------|
| `config.bin` | Client ID, Client Secret, Calendar ID (DPAPI-encrypted) |
| `refresh_token.bin` | OAuth refresh token (DPAPI-encrypted) |
| `window.json` | Window geometry, theme, position flag |
| `debug.log` | Rotating log (5 MB, single generation) |

Legacy plaintext files (`config.json`, `refresh_token.txt`) from earlier versions are migrated automatically on first launch.

### Re-authenticating

If you need to sign in with a different Google account, open the tray menu → **Quit**, delete `refresh_token.bin` from the config folder, and restart the widget. The setup screen will not reappear (credentials are still valid), and a new OAuth flow will start automatically.

To reset everything, delete the whole `%APPDATA%\example\gcal-widget\` folder.

---

## Troubleshooting

| Symptom | Fix |
|---------|-----|
| Browser shows `Error 403: access_denied` | You forgot to add your account as a test user in Step 3. |
| `invalid_client` during authorization | Wrong Client ID / Client Secret. Use **Configure credentials** from the error screen to re-enter them. |
| No events appear | The calendar is empty in the selected range, or you entered the wrong Calendar ID. Try `primary`. |
| Widget disappears after Win+D | Ensure you are on Windows 11 build 22000 or later. |
| Events shifted by one hour after a DST change | Report it as a bug — DST handling is explicitly supported. |

---

## Building from source

Install [Rust](https://rustup.rs/) (stable toolchain) and the [Visual Studio C++ build tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), then:

cargo build --release


The binary is produced in `target/release/google-calendar-widget.exe`.

For development:

cargo run


---

## What's new in 1.3.0

- **DST-safe drag & drop:** moving an event across a daylight-saving boundary now preserves the local wall-clock time (10:00 stays 10:00) and the correct all-day date.
- **Recurring series updates:** editing "All events" now correctly propagates changes to the end time / duration, not just the start.
- **"This and following" split:** the new tail of a split series keeps the original `UNTIL` boundary, so a bounded series no longer silently becomes infinite.
- **Recurrence end validation:** the *Until* date is rejected if it falls before the event's start date.
- **Undo safety:** the undo banner is only offered for non-recurring events.
- **Window startup:** the first-run window position is no longer baked in as `(0, 0)`.
- **Minimize handling:** a `(0, 0)` resize (tray minimize) is no longer treated as a real size.
- **Startup visibility:** the delayed hide timer no longer contradicts `reveal_window()`, avoiding a brief flash on launch.
- **Empty-title confirmation** can no longer be bypassed by clicking **Save** twice.
- **DST gaps:** `local_to_utc` and all-day parsing fall back to the next valid local time instead of silently shifting the event by a full timezone offset.
- **OAuth callback** decodes `+` as space in `error_description`.
- **UI:** color swatches resized so the form fits the minimum window width (360 px).

---

## License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for the full text.

Copyright 2025 Google Calendar Widget contributors

Unless required by applicable law or agreed to in writing, software distributed under the License is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied. See the License for the specific language governing permissions and limitations under the License.
