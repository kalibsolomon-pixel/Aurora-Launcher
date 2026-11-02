// The standalone review fixture has no native event bus or install transactions.
export async function listen(): Promise<() => void> { return () => {}; }
