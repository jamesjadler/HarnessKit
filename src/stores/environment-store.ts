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
      pending += 1;
      set({ busy: true });
      const job = queue.then(async () => {
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
          set({ busy: pending > 0 });
        }
      });
      queue = job.catch(() => {});
      return job;
    },
  };
});
