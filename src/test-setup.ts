import "@testing-library/jest-dom/vitest";
import { cleanup } from "@testing-library/react";
import { clearMocks } from "@tauri-apps/api/mocks";
import { afterEach } from "vitest";

afterEach(async () => {
  cleanup();
  // Unmount cleanups (event unlisten) resolve asynchronously and still need the mocked internals.
  await new Promise((resolve) => setTimeout(resolve, 0));
  clearMocks();
});
