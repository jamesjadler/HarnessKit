import type {
  CreateKitRequest,
  KitAssetCandidates,
  KitConflictPreview,
  KitDetails,
  KitSummary,
  KitSyncResult,
  PreviewKitConflictsRequest,
  ProjectInstallRecords,
  SyncKitRequest,
  UnsyncKitRequest,
  UpdateKitRequest,
} from "@/types/kits";
import { transport } from "./transport";
import type {
  AgentDetail,
  AgentInfo,
  AuditResult,
  ConfigScope,
  DashboardStats,
  DiscoveredProject,
  Extension,
  ExtensionContent,
  FileEntry,
  InstallRepositoryItemRequest,
  InstallResult,
  MarketplaceItem,
  Project,
  RepositoryWithItems,
  ScanResult,
  SkillAuditInfo,
  ToggleOutcome,
  UpdateStatus,
} from "./types";

function validateNonEmpty(value: string, label: string): void {
  if (!value?.trim()) {
    throw new Error(`${label} cannot be empty`);
  }
}

async function environmentAction<T>(
  id: string,
  action: string,
  enabled?: boolean,
  expected?: string | null,
): Promise<T> {
  const { useEnvironmentStore } = await import("@/stores/environment-store");
  const state = useEnvironmentStore.getState();
  const member = id.slice("environment:".length);
  const copy = state.data?.registry.skills
    .flatMap((s) => s.members)
    .find((m) => m.id === member);
  if (!copy) throw new Error("Environment inventory changed; refresh first");
  const result = await transport<T>("environment_extension", {
    member,
    action,
    expected: expected === undefined ? copy.fingerprint : expected,
    enabled: enabled ?? null,
  });
  if (action !== "content") await state.command({ action: "overview" });
  return result;
}

async function trackedAction<T>(
  id: string,
  action: string,
  fallback: () => Promise<T>,
  enabled?: boolean,
): Promise<T> {
  if (id.startsWith("environment:"))
    return environmentAction(id, action, enabled);
  const { useExtensionStore } = await import("@/stores/extension-store");
  const extension = useExtensionStore
    .getState()
    .extensions.find((e) => e.id === id);
  return extension?.sync_member_id
    ? environmentAction(
        `environment:${extension.sync_member_id}`,
        action,
        enabled,
        extension.sync_revision,
      )
    : fallback();
}

export const api = {
  listExtensions(kind?: string, agent?: string): Promise<Extension[]> {
    return transport("list_extensions", { kind, agent });
  },

  listAgents(): Promise<AgentInfo[]> {
    return transport("list_agents");
  },

  getDashboardStats(): Promise<DashboardStats> {
    return transport("get_dashboard_stats");
  },

  toggleExtension(id: string, enabled: boolean): Promise<ToggleOutcome> {
    validateNonEmpty(id, "Extension ID");
    return trackedAction(
      id,
      "toggle",
      () => transport("toggle_extension", { id, enabled }),
      enabled,
    );
  },

  listAuditResults(): Promise<AuditResult[]> {
    return transport("list_audit_results");
  },

  runAudit(): Promise<AuditResult[]> {
    return transport("run_audit");
  },

  scanAndSync(): Promise<number> {
    return transport("scan_and_sync");
  },

  deleteExtension(
    id: string,
    observed?: { member: string; fingerprint: string | null },
  ): Promise<void> {
    if (observed)
      return environmentAction(
        `environment:${observed.member}`,
        "delete",
        undefined,
        observed.fingerprint,
      );
    validateNonEmpty(id, "Extension ID");
    return trackedAction(id, "delete", () =>
      transport("delete_extension", { id }),
    );
  },

  uninstallCliBinary(binaryPath: string): Promise<void> {
    return transport("uninstall_cli_binary", { binaryPath });
  },

  getExtensionContent(id: string): Promise<ExtensionContent> {
    return trackedAction(id, "content", () =>
      transport("get_extension_content", { id }),
    );
  },

  getCachedUpdateStatuses(): Promise<[string, UpdateStatus][]> {
    return transport("get_cached_update_statuses");
  },

  getSkillLocations(name: string): Promise<[string, string, string | null][]> {
    return transport("get_skill_locations", { name });
  },

  checkUpdates(): Promise<[string, UpdateStatus][]> {
    return transport("check_updates");
  },

  updateExtension(id: string): Promise<InstallResult> {
    return transport("update_extension", { id });
  },

  installFromLocal(
    path: string,
    targetAgents: string[],
    targetScope: ConfigScope,
    hermesCategory?: string,
  ): Promise<InstallResult> {
    return transport("install_from_local", {
      path,
      targetAgents,
      targetScope,
      hermesCategory: hermesCategory ?? null,
    });
  },

  listHermesCategories(): Promise<string[]> {
    return transport("list_hermes_categories");
  },

  scanGitRepo(
    url: string,
    targetAgents: string[],
    targetScope: ConfigScope,
    hermesCategory?: string,
  ): Promise<ScanResult> {
    return transport("scan_git_repo", {
      url,
      targetAgents,
      targetScope,
      hermesCategory: hermesCategory ?? null,
    });
  },

  installScannedSkills(
    cloneId: string,
    skillIds: string[],
    targetAgents: string[],
    targetScope: ConfigScope,
    track: boolean,
    hermesCategory?: string,
  ): Promise<InstallResult[]> {
    return transport("install_scanned_skills", {
      cloneId,
      skillIds,
      targetAgents,
      targetScope,
      hermesCategory: hermesCategory ?? null,
      track,
    });
  },

  updateTags(id: string, tags: string[]): Promise<void> {
    return transport("update_tags", { id, tags });
  },

  getAllTags(): Promise<string[]> {
    return transport("get_all_tags");
  },

  updatePack(id: string, pack: string | null): Promise<void> {
    return transport("update_pack", { id, pack });
  },

  batchUpdateTags(ids: string[], tags: string[]): Promise<void> {
    return transport("batch_update_tags", { ids, tags });
  },

  batchUpdatePack(ids: string[], pack: string | null): Promise<void> {
    return transport("batch_update_pack", { ids, pack });
  },

  getAllPacks(): Promise<string[]> {
    return transport("get_all_packs");
  },

  searchMarketplace(
    query: string,
    kind: string,
    limit?: number,
  ): Promise<MarketplaceItem[]> {
    return transport("search_marketplace", { query, kind, limit });
  },

  trendingMarketplace(
    kind: string,
    limit?: number,
  ): Promise<MarketplaceItem[]> {
    return transport("trending_marketplace", { kind, limit });
  },

  fetchSkillPreview(
    source: string,
    skillId: string,
    gitUrl?: string | null,
  ): Promise<string> {
    return transport("fetch_skill_preview", {
      source,
      skillId,
      gitUrl: gitUrl ?? null,
    });
  },

  fetchCliReadme(source: string): Promise<string> {
    return transport("fetch_cli_readme", { source });
  },

  fetchSkillAudit(
    source: string,
    skillId: string,
  ): Promise<SkillAuditInfo | null> {
    return transport("fetch_skill_audit", { source, skillId });
  },

  installFromMarketplace(
    source: string,
    skillId: string,
    targetAgent: string,
    targetScope: ConfigScope,
    hermesCategory?: string,
  ): Promise<InstallResult> {
    return transport("install_from_marketplace", {
      source,
      skillId,
      targetAgent,
      targetScope,
      hermesCategory: hermesCategory ?? null,
    });
  },

  installToAgent(
    extensionId: string,
    targetAgent: string,
    targetScope: ConfigScope,
    hermesCategory?: string,
  ): Promise<string> {
    return transport("install_to_agent", {
      extensionId,
      targetAgent,
      targetScope,
      hermesCategory: hermesCategory ?? null,
    });
  },

  listProjects(): Promise<Project[]> {
    return transport("list_projects");
  },

  addProject(path: string): Promise<Project> {
    return transport("add_project", { path });
  },

  removeProject(id: string): Promise<void> {
    return transport("remove_project", { id });
  },

  discoverProjects(rootPath: string): Promise<DiscoveredProject[]> {
    return transport("discover_projects", { rootPath });
  },

  updateAgentPath(name: string, path: string | null): Promise<void> {
    return transport("update_agent_path", { name, path });
  },

  setAgentEnabled(name: string, enabled: boolean): Promise<void> {
    return transport("set_agent_enabled", { name, enabled });
  },

  listSkillFiles(path: string): Promise<FileEntry[]> {
    return transport("list_skill_files", { path });
  },

  openInSystem(path: string): Promise<void> {
    return transport("open_in_system", { path });
  },

  revealInFileManager(path: string): Promise<void> {
    return transport("reveal_in_file_manager", { path });
  },

  listAgentConfigs(): Promise<AgentDetail[]> {
    return transport("list_agent_configs");
  },

  readConfigFilePreview(path: string, maxLines?: number): Promise<string> {
    return transport("read_config_file_preview", { path, maxLines });
  },

  addCustomConfigPath(
    agent: string,
    path: string,
    label: string,
    category: string,
    targetScope: ConfigScope,
  ): Promise<number> {
    return transport("add_custom_config_path", {
      agent,
      path,
      label,
      category,
      targetScope,
    });
  },

  updateCustomConfigPath(
    id: number,
    path: string,
    label: string,
    category: string,
  ): Promise<void> {
    return transport("update_custom_config_path", {
      id,
      path,
      label,
      category,
    });
  },

  removeCustomConfigPath(id: number): Promise<void> {
    return transport("remove_custom_config_path", { id });
  },

  updateAgentOrder(names: string[]): Promise<void> {
    return transport("update_agent_order", { names });
  },

  getCliWithChildren(cliId: string): Promise<[Extension, Extension[]]> {
    return transport("get_cli_with_children", { cliId });
  },

  listCliMarketplace(): Promise<MarketplaceItem[]> {
    return transport("list_cli_marketplace");
  },

  setAppIcon(name: string): Promise<void> {
    return transport("set_app_icon", { name });
  },

  /** Windows only: colors for the native title bar (`#rrggbb`). */
  setTitleBarColor(background: string, text: string): Promise<void> {
    return transport("set_title_bar_color", { background, text });
  },

  listRepositories(): Promise<RepositoryWithItems[]> {
    return transport("list_repositories");
  },
  addRepository(url: string): Promise<string> {
    return transport("add_repository", { url });
  },
  syncRepository(id: string): Promise<void> {
    return transport("sync_repository", { id });
  },
  deleteRepository(id: string): Promise<void> {
    return transport("delete_repository", { id });
  },
  ignoreRepositoryItem(
    repoId: string,
    itemId: string,
    ignored: boolean,
  ): Promise<void> {
    return transport("ignore_repository_item", {
      req: { repo_id: repoId, item_id: itemId, ignored },
    });
  },
  getRepositoryItemContent(repoId: string, itemId: string): Promise<string> {
    return transport("get_repository_item_content", { repoId, itemId });
  },
  installRepositoryItem(req: InstallRepositoryItemRequest): Promise<void> {
    return transport("install_repository_item", { req });
  },
  listKits(): Promise<KitSummary[]> {
    return transport("list_kits");
  },
  getKitDetails(id: string): Promise<KitDetails> {
    return transport("get_kit_details", { id });
  },
  listKitAssetCandidates(): Promise<KitAssetCandidates> {
    return transport("list_kit_asset_candidates");
  },
  createKit(req: CreateKitRequest): Promise<KitSummary> {
    return transport("create_kit", { req });
  },
  updateKit(req: UpdateKitRequest): Promise<KitSummary> {
    return transport("update_kit", { req });
  },
  deleteKit(id: string): Promise<void> {
    return transport("delete_kit", { id });
  },
  previewKitProjectConflicts(
    req: PreviewKitConflictsRequest,
  ): Promise<KitConflictPreview> {
    return transport("preview_kit_project_conflicts", { req });
  },
  syncKitToProject(req: SyncKitRequest): Promise<KitSyncResult> {
    return transport("sync_kit_to_project", { req });
  },
  unsyncKitFromProject(req: UnsyncKitRequest): Promise<void> {
    return transport("unsync_kit_from_project", { req });
  },
  exportKit(id: string, targetPath: string): Promise<void> {
    return transport("export_kit", { id, targetPath });
  },
  importKit(sourceZipPath: string): Promise<KitSummary> {
    return transport("import_kit", { sourceZipPath });
  },
  listProjectInstallRecords(): Promise<ProjectInstallRecords[]> {
    return transport("list_project_install_records");
  },
};
