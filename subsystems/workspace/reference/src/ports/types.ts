/** Source-location vocabulary shared across ports. Lines and columns are 1-based. */

export interface SourceLocation {
  readonly path: string;
  readonly line: number;
  readonly column: number;
}
