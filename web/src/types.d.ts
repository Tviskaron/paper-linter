declare module "js-untar" {
  export interface UntarRecord {
    name: string;
    buffer?: ArrayBuffer;
    blob?: Blob;
  }

  export default function untar(buffer: ArrayBuffer): Promise<UntarRecord[]>;
}

declare module "../pkg/paper_linter.js" {
  export default function init(): Promise<unknown>;
  export class PaperLinter {
    constructor();
    add_file(path: string, bytes: Uint8Array): void;
    check(optionsJson: string): string;
    rules_json(): string;
  }
}
