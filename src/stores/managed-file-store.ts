import { create } from "zustand";
import { transport } from "@/lib/transport";
import type {
  ManagedFile,
  ManagedFileRequest,
  ManagedFiles,
} from "@/lib/managed-files";
interface State {
  files: ManagedFile[];
  loaded: boolean;
  loading: boolean;
  busy: boolean;
  error: string | null;
  refresh: () => Promise<void>;
  command: (req: ManagedFileRequest) => Promise<ManagedFiles>;
}
let generation = 0;
export const useManagedFileStore = create<State>((set, get) => ({
  files: [],
  loaded: false,
  loading: false,
  busy: false,
  error: null,
  async refresh() {
    if (get().busy || get().loading) return;
    const request = ++generation;
    set({ loading: true });
    try {
      const result = await transport<ManagedFiles>("managed_file_command", {
        req: { action: "list" },
      });
      if (request === generation)
        set({ files: result.files, loaded: true, error: null });
    } catch (e) {
      if (request === generation) set({ error: String(e) });
    } finally {
      if (request === generation) set({ loading: false });
    }
  },
  async command(req) {
    if (get().busy) throw new Error("A file operation is already running");
    ++generation;
    set({ busy: true, loading: false, error: null });
    try {
      const result = await transport<ManagedFiles>("managed_file_command", {
        req,
      });
      set({ files: result.files, loaded: true });
      return result;
    } catch (e) {
      set({ error: String(e) });
      throw e;
    } finally {
      set({ busy: false });
    }
  },
}));
