import { expect, it, vi } from "vitest";
import { api } from "../invoke";
import { transport } from "../transport";

vi.mock("../transport", () => ({
  transport: vi.fn().mockResolvedValue({ redacted_secret_keys: [] }),
}));
vi.mock("@/stores/extension-store", () => ({
  useExtensionStore: {
    getState: () => ({
      extensions: [{ id: "native-local-id", sync_member_id: "member-1" }],
    }),
  },
}));
vi.mock("@/stores/environment-store", () => ({
  useEnvironmentStore: {
    getState: () => ({
      data: {
        registry: {
          skills: [{ members: [{ id: "member-1", fingerprint: "observed" }] }],
        },
      },
      command: vi.fn().mockResolvedValue(undefined),
    }),
  },
}));
it("routes tracked native IDs through coordinator stale-content checks", async () => {
  await api.toggleExtension("native-local-id", false);
  expect(transport).toHaveBeenCalledWith("environment_extension", {
    member: "member-1",
    action: "toggle",
    expected: "observed",
    enabled: false,
  });
});
