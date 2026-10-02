import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import type { Transport } from "codemirror-languageserver";

export class TauriLspTransport implements Transport {
  private messageHandler: ((message: string) => void) | undefined;
  private closeHandler: (() => void) | undefined;
  private errorHandler: ((error: Error) => void) | undefined;
  private unlisten: UnlistenFn | undefined;
  private sessionId: string;
  private sendQueue: Promise<void> = Promise.resolve();

  constructor(sessionId: string) { this.sessionId = sessionId; }

  setUnlisten(unlisten: UnlistenFn) { this.unlisten = unlisten; }
  send(message: string) {
    this.sendQueue = this.sendQueue
      .then(() => invoke<void>("send_clangd", { sessionId: this.sessionId, message }))
      .catch((error) => { this.errorHandler?.(new Error(String(error))); });
  }
  onMessage(callback: (message: string) => void) { this.messageHandler = callback; }
  onClose(callback: () => void) { this.closeHandler = callback; }
  onError(callback: (error: Error) => void) { this.errorHandler = callback; }
  receive(message: string) {
    if (message === "__clangd_closed__") this.closeHandler?.();
    else if (message.startsWith("__clangd_error__:")) this.errorHandler?.(new Error(message));
    else this.messageHandler?.(message);
  }
  close() {
    this.unlisten?.();
    this.unlisten = undefined;
    this.sendQueue = this.sendQueue
      .then(() => invoke<void>("stop_clangd", { sessionId: this.sessionId }))
      .catch(() => {});
    this.closeHandler?.();
  }
}
