import { useEffect, useMemo, useState } from "react";
import {
  backupRestoreApks,
  loadRestoreProfile,
  restoreLocalApks,
  saveRestoreProfile,
  scanRestoreProfile,
  verifyRestorePackages,
  type ApkBackupReport,
  type LocalRestoreReport,
  type RestoreApp,
  type RestoreProfile,
  type RestoreProfileConfig,
  type RestoreStrategy,
  type RestoreVerification,
} from "../lib/tauri";

interface RestorePanelProps {
  serial: string | null;
  onLog: (message: string) => void;
}

type ManagedRestoreApp = RestoreApp & { enabled: boolean };
type RestoreFilter = "all" | "google_play" | "source_manager" | "local_apk_backup";

const strategyOptions: Array<{ value: RestoreStrategy; label: string }> = [
  { value: "google_play", label: "Google Play" },
  { value: "source_manager", label: "Source manager" },
  { value: "local_apk_backup", label: "Local APK backup" },
  { value: "manual", label: "Manual" },
  { value: "skip", label: "Skip" },
];

function sourceLabel(source: string) {
  const labels: Record<string, string> = {
    google_play: "Google Play",
    obtainium: "Obtainium",
    fdroid: "F-Droid",
    aurora_store: "Aurora Store",
    amazon_appstore: "Amazon Appstore",
    galaxy_store: "Galaxy Store",
    huawei_appgallery: "Huawei AppGallery",
    local_or_adb: "Local / ADB",
    external_or_unknown: "External / unknown",
    unknown: "Unknown",
  };
  return labels[source] ?? source;
}

function strategyLabel(strategy: RestoreStrategy) {
  return strategyOptions.find((item) => item.value === strategy)?.label ?? strategy;
}

function profileCounts(apps: ManagedRestoreApp[]) {
  return {
    total: apps.length,
    googlePlay: apps.filter((app) => app.restoreStrategy === "google_play").length,
    sourceManager: apps.filter((app) => app.restoreStrategy === "source_manager").length,
    localApkBackup: apps.filter((app) => app.restoreStrategy === "local_apk_backup").length,
  };
}

function RestorePanel({ serial, onLog }: RestorePanelProps) {
  const [profile, setProfile] = useState<RestoreProfile | null>(null);
  const [apps, setApps] = useState<ManagedRestoreApp[]>([]);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState<RestoreFilter>("all");
  const [backupDirectory, setBackupDirectory] = useState("");
  const [scanBusy, setScanBusy] = useState(false);
  const [saveBusy, setSaveBusy] = useState(false);
  const [loadBusy, setLoadBusy] = useState(false);
  const [backupBusy, setBackupBusy] = useState(false);
  const [restoreBusy, setRestoreBusy] = useState(false);
  const [verifyBusy, setVerifyBusy] = useState(false);
  const [backupReport, setBackupReport] = useState<ApkBackupReport | null>(null);
  const [restoreReport, setRestoreReport] = useState<LocalRestoreReport | null>(null);
  const [verification, setVerification] = useState<RestoreVerification | null>(null);

  useEffect(() => {
    setBackupReport(null);
    setRestoreReport(null);
    setVerification(null);
  }, [serial]);

  const enabledApps = useMemo(
    () => apps.filter((app) => app.enabled && app.restoreStrategy !== "skip"),
    [apps],
  );

  const localBackupPackages = useMemo(
    () =>
      enabledApps
        .filter((app) => app.restoreStrategy === "local_apk_backup")
        .map((app) => app.packageName),
    [enabledApps],
  );

  const configuredCounts = useMemo(() => {
    const count = (strategy: RestoreStrategy) =>
      enabledApps.filter((app) => app.restoreStrategy === strategy).length;
    return {
      total: enabledApps.length,
      googlePlay: count("google_play"),
      sourceManager: count("source_manager"),
      localBackup: count("local_apk_backup"),
      manual: count("manual"),
    };
  }, [enabledApps]);

  const visibleApps = useMemo(() => {
    const normalizedQuery = query.trim().toLowerCase();
    return apps.filter((app) => {
      const filterMatch = filter === "all" || app.restoreStrategy === filter;
      const queryMatch =
        !normalizedQuery ||
        app.packageName.toLowerCase().includes(normalizedQuery) ||
        (app.installerPackage ?? "").toLowerCase().includes(normalizedQuery) ||
        sourceLabel(app.sourceKind).toLowerCase().includes(normalizedQuery);
      return filterMatch && queryMatch;
    });
  }, [apps, filter, query]);

  function updateApp(packageName: string, patch: Partial<ManagedRestoreApp>) {
    setApps((current) =>
      current.map((app) => (app.packageName === packageName ? { ...app, ...patch } : app)),
    );
    setBackupReport(null);
    setRestoreReport(null);
    setVerification(null);
  }

  function currentProfileConfig(): RestoreProfileConfig | null {
    if (!profile) return null;
    return {
      version: 1,
      deviceProduct: profile.deviceProduct,
      androidRelease: profile.androidRelease,
      sdkLevel: profile.sdkLevel,
      apps: apps.map((app) => ({
        packageName: app.packageName,
        installerPackage: app.installerPackage,
        sourceKind: app.sourceKind,
        restoreStrategy: app.restoreStrategy,
        enabled: app.enabled,
      })),
    };
  }

  async function scanProfile() {
    if (!serial || scanBusy) return;
    setScanBusy(true);
    try {
      const result = await scanRestoreProfile(serial);
      setProfile(result);
      setApps(result.apps.map((app) => ({ ...app, enabled: app.enabledByDefault })));
      setBackupReport(null);
      setRestoreReport(null);
      setVerification(null);
      onLog(result.diagnostic);
    } catch (error) {
      onLog(`Restore profile scan failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setScanBusy(false);
    }
  }

  async function saveProfile(quiet = false) {
    const config = currentProfileConfig();
    if (!config || !backupDirectory.trim() || saveBusy) return false;
    setSaveBusy(true);
    try {
      const result = await saveRestoreProfile(backupDirectory.trim(), config);
      if (!quiet) onLog(result.diagnostic);
      return true;
    } catch (error) {
      onLog(`Restore profile save failed: ${error instanceof Error ? error.message : String(error)}`);
      return false;
    } finally {
      setSaveBusy(false);
    }
  }

  async function loadProfile() {
    if (!backupDirectory.trim() || loadBusy) return;
    setLoadBusy(true);
    try {
      const result = await loadRestoreProfile(backupDirectory.trim());
      const loadedApps: ManagedRestoreApp[] = result.apps.map((app) => ({
        packageName: app.packageName,
        installerPackage: app.installerPackage,
        sourceKind: app.sourceKind,
        restoreStrategy: app.restoreStrategy,
        enabledByDefault: app.enabled,
        enabled: app.enabled,
      }));
      setProfile({
        version: result.version,
        serial: serial ?? "saved-profile",
        deviceProduct: result.deviceProduct,
        androidRelease: result.androidRelease,
        sdkLevel: result.sdkLevel,
        apps: loadedApps,
        counts: profileCounts(loadedApps),
        diagnostic: `Loaded ${loadedApps.length} app(s) from flashrom-restore-profile.json.`,
      });
      setApps(loadedApps);
      setBackupReport(null);
      setRestoreReport(null);
      setVerification(null);
      onLog(`Loaded restore profile with ${loadedApps.length} app(s).`);
    } catch (error) {
      onLog(`Restore profile load failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setLoadBusy(false);
    }
  }

  async function backupLocalPackages() {
    if (!serial || !backupDirectory.trim() || localBackupPackages.length === 0 || backupBusy) return;
    const saved = await saveProfile(true);
    if (!saved) return;

    setBackupBusy(true);
    try {
      const result = await backupRestoreApks({
        serial,
        destination: backupDirectory.trim(),
        packages: localBackupPackages,
      });
      setBackupReport(result);
      onLog(result.diagnostic);
    } catch (error) {
      setBackupReport(null);
      onLog(`APK backup failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setBackupBusy(false);
    }
  }

  async function restoreLocalPackages() {
    if (!serial || !backupDirectory.trim() || localBackupPackages.length === 0 || restoreBusy) return;
    setRestoreBusy(true);
    try {
      const result = await restoreLocalApks({
        serial,
        sourceDirectory: backupDirectory.trim(),
        packages: localBackupPackages,
      });
      setRestoreReport(result);
      onLog(result.diagnostic);
    } catch (error) {
      setRestoreReport(null);
      onLog(`Local APK restore failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setRestoreBusy(false);
    }
  }

  async function verifyProfile() {
    if (!serial || enabledApps.length === 0 || verifyBusy) return;
    setVerifyBusy(true);
    try {
      const result = await verifyRestorePackages(
        serial,
        enabledApps.map((app) => app.packageName),
      );
      setVerification(result);
      onLog(result.diagnostic);
    } catch (error) {
      setVerification(null);
      onLog(`Restore verification failed: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setVerifyBusy(false);
    }
  }

  return (
    <section className="panel restore-panel">
      <div className="section-heading">
        <div>
          <p className="eyebrow">Post-ROM recovery</p>
          <h2>App Restore Profile</h2>
        </div>
        <p>
          Scan third-party apps before a clean flash, keep Google Play apps delegated to Android restore,
          and preserve local/sideloaded APKs separately.
        </p>
      </div>

      <div className="restore-toolbar">
        <div className="restore-device-copy">
          <span>ADB device</span>
          <strong>{serial ?? "No detected serial"}</strong>
          <small>Scanning, APK backup and restore require normal Android ADB state.</small>
        </div>
        <button
          type="button"
          className="button button-primary"
          disabled={!serial || scanBusy}
          onClick={() => void scanProfile()}
        >
          {scanBusy ? "Scanning apps…" : "Scan Installed Apps"}
        </button>
      </div>

      <div className="restore-profile-storage">
        <div>
          <span>Restore workspace</span>
          <strong>flashrom-restore-profile.json + local APK folders</strong>
        </div>
        <input
          value={backupDirectory}
          onChange={(event) => setBackupDirectory(event.target.value)}
          placeholder="D:\\FlashROM-Backup\\apps"
          spellCheck={false}
          autoComplete="off"
        />
        <button
          type="button"
          className="button button-secondary"
          disabled={!backupDirectory.trim() || loadBusy}
          onClick={() => void loadProfile()}
        >
          {loadBusy ? "Loading…" : "Load Profile"}
        </button>
        <button
          type="button"
          className="button button-secondary"
          disabled={!profile || !backupDirectory.trim() || saveBusy}
          onClick={() => void saveProfile()}
        >
          {saveBusy ? "Saving…" : "Save Profile"}
        </button>
      </div>

      {profile && (
        <div className="restore-profile-result">
          <div className="restore-summary-grid">
            <div>
              <span>Device</span>
              <strong>{profile.deviceProduct ?? "Unknown"}</strong>
              <small>Android {profile.androidRelease ?? "?"} / SDK {profile.sdkLevel ?? "?"}</small>
            </div>
            <div>
              <span>Enabled apps</span>
              <strong>{configuredCounts.total}</strong>
              <small>{profile.apps.length} in profile</small>
            </div>
            <div>
              <span>Google Play</span>
              <strong>{configuredCounts.googlePlay}</strong>
              <small>Restore during Android setup</small>
            </div>
            <div>
              <span>Source manager</span>
              <strong>{configuredCounts.sourceManager}</strong>
              <small>Obtainium / F-Droid / external stores</small>
            </div>
            <div>
              <span>Local APK</span>
              <strong>{configuredCounts.localBackup}</strong>
              <small>Back up before clean flash</small>
            </div>
            <div>
              <span>Manual</span>
              <strong>{configuredCounts.manual}</strong>
              <small>User-managed restore</small>
            </div>
          </div>

          <p className="restore-diagnostic">{profile.diagnostic}</p>

          <div className="restore-filters">
            <input
              type="search"
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder="Search package or installer…"
            />
            <select value={filter} onChange={(event) => setFilter(event.target.value as RestoreFilter)}>
              <option value="all">All strategies</option>
              <option value="google_play">Google Play</option>
              <option value="source_manager">Source manager</option>
              <option value="local_apk_backup">Local APK backup</option>
            </select>
            <span>{visibleApps.length} visible</span>
          </div>

          <div className="restore-app-list">
            {visibleApps.map((app) => (
              <article className={`restore-app-row ${app.enabled ? "" : "restore-app-disabled"}`} key={app.packageName}>
                <label className="restore-app-enable">
                  <input
                    type="checkbox"
                    checked={app.enabled}
                    onChange={(event) => updateApp(app.packageName, { enabled: event.target.checked })}
                  />
                  <span>Use</span>
                </label>
                <div className="restore-app-identity">
                  <code>{app.packageName}</code>
                  <span>{sourceLabel(app.sourceKind)}</span>
                  <small>{app.installerPackage ?? "Installer not reported"}</small>
                </div>
                <label className="restore-strategy-select">
                  <span>Restore strategy</span>
                  <select
                    value={app.restoreStrategy}
                    disabled={!app.enabled}
                    onChange={(event) =>
                      updateApp(app.packageName, {
                        restoreStrategy: event.target.value as RestoreStrategy,
                      })
                    }
                  >
                    {strategyOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              </article>
            ))}
          </div>

          <div className="restore-backup-card">
            <div>
              <span>Local APK backup</span>
              <strong>{localBackupPackages.length} package(s) selected</strong>
              <small>
                FlashROM pulls base.apk and split APKs only for apps configured as {strategyLabel("local_apk_backup")}.
              </small>
            </div>
            <div className="restore-backup-actions">
              <button
                type="button"
                className="button button-secondary"
                disabled={!serial || !backupDirectory.trim() || localBackupPackages.length === 0 || backupBusy || saveBusy}
                onClick={() => void backupLocalPackages()}
              >
                {backupBusy ? "Backing up…" : "Backup Local APKs"}
              </button>
              <button
                type="button"
                className="button button-secondary"
                disabled={!serial || !backupDirectory.trim() || localBackupPackages.length === 0 || restoreBusy}
                onClick={() => void restoreLocalPackages()}
              >
                {restoreBusy ? "Restoring…" : "Restore Local APKs"}
              </button>
              <button
                type="button"
                className="button button-secondary"
                disabled={!serial || enabledApps.length === 0 || verifyBusy}
                onClick={() => void verifyProfile()}
              >
                {verifyBusy ? "Verifying…" : "Verify Restored Apps"}
              </button>
            </div>
          </div>

          {backupReport && (
            <div className={`restore-report ${backupReport.failureCount ? "restore-report-warning" : "restore-report-success"}`}>
              <strong>APK Backup</strong>
              <span>{backupReport.diagnostic}</span>
              <small>{backupReport.destination}</small>
              {backupReport.packages.some((item) => !item.success) && (
                <div className="restore-report-items">
                  {backupReport.packages
                    .filter((item) => !item.success)
                    .slice(0, 12)
                    .map((item) => (
                      <code key={item.packageName}>{item.packageName}: {item.diagnostic}</code>
                    ))}
                </div>
              )}
            </div>
          )}

          {restoreReport && (
            <div className={`restore-report ${restoreReport.failureCount ? "restore-report-warning" : "restore-report-success"}`}>
              <strong>Local APK Restore</strong>
              <span>{restoreReport.diagnostic}</span>
              {restoreReport.packages.some((item) => !item.success) && (
                <div className="restore-report-items">
                  {restoreReport.packages
                    .filter((item) => !item.success)
                    .slice(0, 12)
                    .map((item) => (
                      <code key={item.packageName}>{item.packageName}: {item.diagnostic}</code>
                    ))}
                </div>
              )}
            </div>
          )}

          {verification && (
            <div className={`restore-report ${verification.missingCount ? "restore-report-warning" : "restore-report-success"}`}>
              <strong>Restore Verification</strong>
              <span>{verification.diagnostic}</span>
              {verification.missing.length > 0 && (
                <div className="restore-report-items">
                  {verification.missing.slice(0, 24).map((packageName) => (
                    <code key={packageName}>{packageName}</code>
                  ))}
                  {verification.missing.length > 24 && (
                    <small>+ {verification.missing.length - 24} more missing package(s)</small>
                  )}
                </div>
              )}
            </div>
          )}

          <div className="restore-policy-note">
            <strong>Current automation boundary</strong>
            <span>
              Google Play installation remains delegated to Android setup/Play Store. Source-manager apps are classified
              but automated Obtainium/F-Droid profile import is the next phase. Private app data is not copied directly.
            </span>
          </div>
        </div>
      )}
    </section>
  );
}

export default RestorePanel;
