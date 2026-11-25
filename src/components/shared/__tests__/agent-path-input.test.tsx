import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import { expect, it, vi } from "vitest";
import {
  AgentPathInput,
  AutoSaveAgentPaths,
} from "@/components/environments/agent-path-input";
import { api } from "@/lib/invoke";

vi.mock("@/lib/invoke", () => ({ api: { updateAgentPath: vi.fn() } }));
vi.mock("@/stores/toast-store", () => ({ toast: { error: vi.fn() } }));
it("saves a later draft after an in-flight save when settings closes", async () => {
  let finish!: () => void;
  vi.mocked(api.updateAgentPath)
    .mockImplementationOnce(
      () =>
        new Promise<void>((r) => {
          finish = r;
        }),
    )
    .mockResolvedValue(undefined);
  const view = render(
    <AutoSaveAgentPaths>
      <AgentPathInput agent="codex-race-test" path="/old" disabled={false} />
    </AutoSaveAgentPaths>,
  );
  const input = screen.getByRole("textbox");
  fireEvent.change(input, { target: { value: "/first" } });
  fireEvent.blur(input);
  await waitFor(() =>
    expect(api.updateAgentPath).toHaveBeenCalledWith(
      "codex-race-test",
      "/first",
    ),
  );
  fireEvent.change(input, { target: { value: "/second" } });
  view.unmount();
  await act(async () => finish());
  await waitFor(() =>
    expect(api.updateAgentPath).toHaveBeenLastCalledWith(
      "codex-race-test",
      "/second",
    ),
  );
});
