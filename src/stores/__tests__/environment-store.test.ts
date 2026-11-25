import { beforeEach, expect, it, vi } from "vitest";
import { transport } from "@/lib/transport";
import { useEnvironmentStore } from "../environment-store";

vi.mock("@/lib/transport", () => ({ transport: vi.fn() }));
beforeEach(() => {
  vi.resetAllMocks();
  useEnvironmentStore.setState({ data: null, error: null, busy: false });
});
it("serializes commands and keeps busy until queued mutations finish", async () => {
  let release!: (value: unknown) => void;
  vi.mocked(transport)
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          release = resolve;
        }),
    )
    .mockResolvedValueOnce({ registry: { generation: 2 } });
  const first = useEnvironmentStore.getState().command({ action: "overview" });
  const second = useEnvironmentStore.getState().command({ action: "pause" });
  await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(1));
  release({ registry: { generation: 1 } });
  await first;
  expect(useEnvironmentStore.getState().busy).toBe(true);
  await second;
  expect(useEnvironmentStore.getState().busy).toBe(false);
  expect(useEnvironmentStore.getState().data?.registry.generation).toBe(2);
});
it("continues the queue after a failed request", async () => {
  vi.mocked(transport)
    .mockRejectedValueOnce(new Error("Offline"))
    .mockResolvedValueOnce({ registry: { generation: 3 } });
  const first = useEnvironmentStore
    .getState()
    .command({ action: "refresh" })
    .catch((e) => e);
  const second = useEnvironmentStore.getState().command({ action: "pause" });
  expect(await first).toBeInstanceOf(Error);
  await second;
  expect(useEnvironmentStore.getState().data?.registry.generation).toBe(3);
  expect(useEnvironmentStore.getState().error).toBe(null);
});

it("keeps polling non-blocking and drops overlapping overview requests", async () => {
  let release!: (value: unknown) => void;
  vi.mocked(transport).mockImplementationOnce(() => new Promise(resolve => { release = resolve; }));
  const first = useEnvironmentStore.getState().command({ action: "overview" });
  await vi.waitFor(() => expect(transport).toHaveBeenCalledTimes(1));
  expect(useEnvironmentStore.getState().busy).toBe(false);
  await useEnvironmentStore.getState().command({ action: "overview" });
  expect(transport).toHaveBeenCalledTimes(1);
  release({ registry: { generation: 4 } });
  await first;
  expect(useEnvironmentStore.getState().busy).toBe(false);
});
