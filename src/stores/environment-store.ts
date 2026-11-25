import { create } from "zustand";
import type { EnvironmentOverview } from "@/lib/environments";
import { transport } from "@/lib/transport";

interface State {
  data: EnvironmentOverview | null;
  error: string | null;
  busy: boolean;
  filter: string;
  command: (req: Record<string, unknown>) => Promise<void>;
  setFilter: (id: string) => void;
}
export const useEnvironmentStore = create<State>((set) => {
  let queue: Promise<void> = Promise.resolve();
  let pending = 0;
  return {
    data: null,
    error: null,
    busy: false,
    filter: "all",
    setFilter: (filter) => set({ filter }),
    command(req) {
      const background = req.action === "overview";
      // Polls are disposable; never accumulate them behind slow operations.
      if (background && pending > 0) return Promise.resolve();
      pending += 1;
      if (!background) set({ busy: true });
      const independent = req.action === "connect_password" || req.action === "connect";
      const job = (independent ? Promise.resolve() : queue).then(async () => {
        set({ error: null });
        try {
          const data = await transport<EnvironmentOverview>(
            "environment_command",
            { req },
          );
          set({ data });
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : String(error),
          });
          throw error;
        } finally {
          pending -= 1;
          if (pending === 0) set({ busy: false });
        }
      });
      if (!independent) queue = job.catch(() => {});
      return job;
    },
  };
});
