declare module "bun:sqlite" {
  type UnwrapParams<T> = T extends readonly [infer U] ? U : T;

  export interface Statement<Params = unknown, Result = unknown> {
    run(params?: UnwrapParams<Params>): { changes: number; lastInsertRowid?: number | bigint };
    get(params?: UnwrapParams<Params>): Result | undefined;
    all(params?: UnwrapParams<Params>): Result[];
  }

  export class Database {
    constructor(filename?: string, options?: { strict?: boolean; readonly?: boolean; create?: boolean });
    prepare<Params = unknown, Result = unknown>(sql: string): Statement<Params, Result>;
    query<Params = unknown, Result = unknown>(sql: string): Statement<Params, Result>;
    exec(sql: string): void;
    pragma(sql: string, options?: { simple?: boolean }): unknown;
    transaction<TArgs extends readonly unknown[], TReturn>(
      fn: (...args: TArgs) => TReturn
    ): (...args: TArgs) => TReturn;
    close(): void;
  }
}
