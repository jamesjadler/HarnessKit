import { clsx } from "clsx";
import { Download, Loader2, RefreshCw } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useSearchParams } from "react-router-dom";
import { EnvironmentManager } from "@/components/environments/environment-manager";
import {
  applyLanguagePreference,
  getStoredLanguagePreference,
  type LanguagePreference,
} from "@/lib/i18n";
import { api } from "@/lib/invoke";
import { isDesktop, isMac } from "@/lib/transport";
import { useAgentStore } from "@/stores/agent-store";
import { toast } from "@/stores/toast-store";
import type { AgentVisibility, AppIcon, ThemeName } from "@/stores/ui-store";
import { useUIStore } from "@/stores/ui-store";
import { useUpdateStore } from "@/stores/update-store";
import { useWebUpdateStore } from "@/stores/web-update-store";

const THEME_OPTIONS: {
  value: ThemeName;
  label: string;
  colors: [string, string, string];
}[] = [
  {
    value: "tiesen",
    label: "Tiesen",
    colors: [
      "oklch(0.5144 0.1605 267.4400)",
      "oklch(0.9851 0 0)",
      "oklch(0 0 0)",
    ],
  },
  {
    value: "claude",
    label: "Claude",
    colors: [
      "oklch(0.6171 0.1375 39.0427)",
      "oklch(0.9665 0.0067 97.3521)",
      "oklch(0.2679 0.0036 106.6427)",
    ],
  },
];

const ICON_OPTIONS: { value: AppIcon; label: string; src: string }[] = [
  { value: "icon-1", label: "Tiesen", src: "/icons/app-icon-1.png" },
  { value: "icon-2", label: "Claude", src: "/icons/app-icon-2.png" },
];

const LANGUAGE_OPTIONS: {
  value: LanguagePreference;
  labelKey:
    | "language.system"
    | "language.en"
    | "language.zh"
    | "language.zh-TW";
}[] = [
  { value: "system", labelKey: "language.system" },
  { value: "en", labelKey: "language.en" },
  { value: "zh", labelKey: "language.zh" },
  { value: "zh-TW", labelKey: "language.zh-TW" },
];

const AGENT_VISIBILITY_OPTIONS: {
  value: AgentVisibility;
  labelKey: "agentPaths.visibilityAll" | "agentPaths.visibilityDetected";
}[] = [
  { value: "all", labelKey: "agentPaths.visibilityAll" },
  { value: "detected", labelKey: "agentPaths.visibilityDetected" },
];

function UpdateSection() {
  const { t } = useTranslation("settings");
  const available = useUpdateStore((s) => s.available);
  const checking = useUpdateStore((s) => s.checking);
  const installing = useUpdateStore((s) => s.installing);
  const checkForUpdate = useUpdateStore((s) => s.checkForUpdate);
  const promptUpdate = useUpdateStore((s) => s.promptUpdate);

  const handleCheck = async () => {
    await checkForUpdate();
    // Show toast if no update found (checked becomes true, available stays null)
    if (!useUpdateStore.getState().available) {
      toast.success(t("update.upToDate"));
    }
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-muted-foreground">v{__APP_VERSION__}</span>
      {available ? (
        <button
          onClick={promptUpdate}
          disabled={installing}
          className="flex items-center gap-1.5 rounded-lg bg-primary px-2.5 py-1 text-xs text-primary-foreground shadow-sm hover:bg-primary/90 disabled:opacity-50 transition-colors"
        >
          {installing ? (
            <Loader2 size={12} className="animate-spin" />
          ) : (
            <Download size={12} />
          )}
          {installing
            ? t("update.updating")
            : t("update.updateTo", { version: available.version })}
        </button>
      ) : (
        <button
          onClick={handleCheck}
          disabled={checking}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-muted disabled:opacity-50 transition-colors"
        >
          <RefreshCw
            size={12}
            className={checking ? "origin-center animate-spin" : ""}
          />
          {checking ? t("update.checking") : t("update.checkForUpdates")}
        </button>
      )}
    </div>
  );
}

function WebUpdateSection() {
  const { t } = useTranslation("settings");
  const available = useWebUpdateStore((s) => s.available);
  const checking = useWebUpdateStore((s) => s.checking);
  const checkForUpdate = useWebUpdateStore((s) => s.checkForUpdate);
  const promptUpdate = useWebUpdateStore((s) => s.promptUpdate);

  const handleCheck = async () => {
    await checkForUpdate(true);
    if (!useWebUpdateStore.getState().available) {
      toast.success(t("update.upToDate"));
    }
  };

  return (
    <div className="flex items-center gap-3">
      <span className="text-xs text-muted-foreground">v{__APP_VERSION__}</span>
      {available ? (
        <button
          onClick={promptUpdate}
          className="flex items-center gap-1.5 rounded-lg bg-primary px-2.5 py-1 text-xs text-primary-foreground shadow-sm hover:bg-primary/90 transition-colors"
        >
          <Download size={12} />
          {t("update.updateTo", { version: available.version })}
        </button>
      ) : (
        <button
          onClick={handleCheck}
          disabled={checking}
          className="flex items-center gap-1.5 rounded-lg border border-border px-2.5 py-1 text-xs text-muted-foreground hover:text-foreground hover:bg-muted disabled:opacity-50 transition-colors"
        >
          <RefreshCw
            size={12}
            className={checking ? "origin-center animate-spin" : ""}
          />
          {checking ? t("update.checking") : t("update.checkForUpdates")}
        </button>
      )}
    </div>
  );
}

export default function SettingsPage() {
  const { t } = useTranslation("settings");
  const languagePreference = getStoredLanguagePreference();
  const {
    themeName,
    mode,
    appIcon,
    agentVisibility,
    autoDisabledAgents,
    setThemeName,
    setMode,
    setAppIcon: setAppIconState,
    setAgentVisibility,
    setAutoDisabledAgents,
  } = useUIStore();
  const { fetch: fetchAgents, setEnabledBulk } = useAgentStore();
  const [searchParams, setSearchParams] = useSearchParams();

  useEffect(() => {
    fetchAgents();
  }, [fetchAgents]);

  useEffect(() => {
    const scrollTo = searchParams.get("scrollTo");
    if (scrollTo) {
      const el = document.getElementById(scrollTo);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", block: "start" });
        searchParams.delete("scrollTo");
        setSearchParams(searchParams, { replace: true });
      }
    }
  }, [searchParams, setSearchParams]);

  // Entering "Detected only" is handled by the App-level reconcile effect: it
  // disables undetected agents and records them in the snapshot (so it also
  // catches agents added by a later app update). Here we only restore that
  // snapshot on the way back to "All agents" — re-enabling exactly those, never
  // agents the user disabled by hand.
  const handleVisibilityChange = (next: AgentVisibility) => {
    if (next === agentVisibility) return;
    if (next === "all") {
      setEnabledBulk(autoDisabledAgents, true);
      setAutoDisabledAgents([]);
    }
    setAgentVisibility(next);
    toast.success(
      t("agentPaths.visibilityToast", {
        label: t(
          AGENT_VISIBILITY_OPTIONS.find((o) => o.value === next)?.labelKey ??
            "agentPaths.visibilityAll",
        ),
      }),
    );
  };

  return (
    <div className="flex flex-1 flex-col min-h-0 -mb-6">
      <div className="shrink-0 pb-4">
        <div className="flex items-center justify-between">
          <h2 className="text-2xl font-bold tracking-tight select-none">
            {t("title")}
          </h2>
          {isDesktop() ? <UpdateSection /> : <WebUpdateSection />}
        </div>
      </div>
      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="w-full max-w-7xl mx-auto space-y-8 pb-6">
          {/* Agent Paths */}
          <section className="space-y-4">
            {/* Header: title + description, with visibility toggle top-right */}
            <div className="flex items-start justify-between gap-4">
              <div>
                <h3 className="text-sm font-medium text-muted-foreground">
                  Environments &amp; Agent Paths
                </h3>
                <p className="text-xs text-muted-foreground mt-1">
                  Expand a machine and environment to manage provider paths.
                </p>
                <p className="text-xs text-muted-foreground mt-1">
                  {t("agentPaths.visibilityHint")}
                </p>
              </div>
              <div className="flex shrink-0 rounded-lg border border-border">
                {AGENT_VISIBILITY_OPTIONS.map((opt, i) => (
                  <button
                    key={opt.value}
                    type="button"
                    onClick={() => handleVisibilityChange(opt.value)}
                    aria-pressed={agentVisibility === opt.value}
                    className={clsx(
                      "px-3 py-1 text-xs font-medium transition-colors duration-200",
                      i === 0 && "rounded-l-lg",
                      i === AGENT_VISIBILITY_OPTIONS.length - 1 &&
                        "rounded-r-lg",
                      agentVisibility === opt.value
                        ? "bg-primary text-primary-foreground shadow-sm"
                        : "text-muted-foreground hover:bg-accent",
                    )}
                  >
                    {t(opt.labelKey)}
                  </button>
                ))}
              </div>
            </div>

            <EnvironmentManager />
          </section>

          {/* Appearance */}
          <section className="space-y-4 border-t border-border pt-8">
            <h3 className="text-sm font-medium text-muted-foreground">
              {t("appearance.title")}
            </h3>

            <div className="flex flex-col gap-2 rounded-lg border border-border bg-card px-4 py-2.5 shadow-sm">
              {/* Theme */}
              <div className="flex items-center justify-between">
                <span className="text-sm">{t("appearance.theme")}</span>
                <div className="flex rounded-lg border border-border">
                  {THEME_OPTIONS.map((theme, i) => (
                    <button
                      key={theme.value}
                      onClick={() => {
                        setThemeName(theme.value);
                        toast.success(
                          t("appearance.themeToast", { label: theme.label }),
                        );
                      }}
                      aria-pressed={themeName === theme.value}
                      className={clsx(
                        "flex items-center gap-1.5 px-3 py-1 text-xs font-medium transition-colors duration-200",
                        i === 0 && "rounded-l-lg",
                        i === THEME_OPTIONS.length - 1 && "rounded-r-lg",
                        themeName === theme.value
                          ? "bg-primary text-primary-foreground shadow-sm"
                          : "text-muted-foreground hover:bg-accent",
                      )}
                    >
                      <span
                        className="h-2.5 w-2.5 rounded-full border border-primary-foreground/20"
                        style={{
                          backgroundColor:
                            themeName === theme.value
                              ? "oklch(1 0 0 / 0.9)"
                              : theme.colors[0],
                        }}
                      />
                      {theme.label}
                    </button>
                  ))}
                </div>
              </div>

              <div className="border-t border-border" />

              {/* Mode */}
              <div className="flex items-center justify-between">
                <span className="text-sm">{t("appearance.mode.label")}</span>
                <div className="flex rounded-lg border border-border">
                  {(["system", "light", "dark"] as const).map((m, i) => (
                    <button
                      key={m}
                      onClick={() => {
                        setMode(m);
                        toast.success(
                          t("appearance.modeToast", {
                            label: t(`appearance.mode.${m}`),
                          }),
                        );
                      }}
                      aria-pressed={mode === m}
                      className={clsx(
                        "px-3 py-1 text-xs font-medium transition-colors duration-200",
                        i === 0 && "rounded-l-lg",
                        i === 2 && "rounded-r-lg",
                        mode === m
                          ? "bg-primary text-primary-foreground shadow-sm"
                          : "text-muted-foreground hover:bg-accent",
                      )}
                    >
                      {t(`appearance.mode.${m}`)}
                    </button>
                  ))}
                </div>
              </div>

              {isDesktop() && isMac() && (
                <>
                  <div className="border-t border-border" />

                  {/* App Icon — macOS only: the command swaps the Dock icon,
                      which has no Windows/Linux equivalent at runtime */}
                  <div className="flex items-center justify-between">
                    <span className="text-sm">{t("appearance.appIcon")}</span>
                    <div className="flex gap-2">
                      {ICON_OPTIONS.map((icon) => (
                        <button
                          key={icon.value}
                          onClick={() => {
                            setAppIconState(icon.value);
                            api
                              .setAppIcon(icon.value)
                              .then(() => {
                                toast.success(
                                  t("appearance.iconToast", {
                                    label: icon.label,
                                  }),
                                );
                              })
                              .catch(() => {
                                toast.error(t("appearance.iconFailed"));
                              });
                          }}
                          aria-pressed={appIcon === icon.value}
                          className={clsx(
                            "rounded-lg p-0.5 transition-all duration-200",
                            appIcon === icon.value
                              ? "ring-2 ring-primary ring-offset-2 ring-offset-card"
                              : "ring-1 ring-border hover:ring-primary/50",
                          )}
                        >
                          <img
                            src={icon.src}
                            alt={icon.label}
                            className="h-10 w-10 rounded-md"
                          />
                        </button>
                      ))}
                    </div>
                  </div>
                </>
              )}
            </div>
          </section>

          {/* Language */}
          <section className="space-y-4 border-t border-border pt-8">
            <div>
              <h3 className="text-sm font-medium text-muted-foreground">
                {t("language.label")}
              </h3>
              <p className="text-xs text-muted-foreground mt-1">
                {t("language.description")}
              </p>
            </div>
            <div className="flex items-center justify-end rounded-lg border border-border bg-card px-4 py-2.5 shadow-sm">
              <div className="flex rounded-lg border border-border">
                {LANGUAGE_OPTIONS.map((opt, i) => (
                  <button
                    key={opt.value}
                    onClick={() => {
                      void applyLanguagePreference(opt.value);
                    }}
                    aria-pressed={languagePreference === opt.value}
                    className={clsx(
                      "px-3 py-1 text-xs font-medium transition-colors duration-200",
                      i === 0 && "rounded-l-lg",
                      i === LANGUAGE_OPTIONS.length - 1 && "rounded-r-lg",
                      languagePreference === opt.value
                        ? "bg-primary text-primary-foreground shadow-sm"
                        : "text-muted-foreground hover:bg-accent",
                    )}
                  >
                    {t(opt.labelKey)}
                  </button>
                ))}
              </div>
            </div>
          </section>

          {/* Footer */}
          <footer className="border-t border-border pt-6 pb-2 flex items-center justify-center gap-1.5 text-xs text-muted-foreground/50">
            <span>HarnessKit</span>
            <span>&middot;</span>
            <span>{t("footer.tagline")}</span>
            <span>&middot;</span>
            <a
              href="https://github.com/RealZST/HarnessKit"
              target="_blank"
              rel="noopener noreferrer"
              className="hover:text-muted-foreground transition-colors"
            >
              {t("footer.github")}
            </a>
          </footer>
        </div>
      </div>
    </div>
  );
}
