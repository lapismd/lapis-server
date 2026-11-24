(() => {
  const invoke = globalThis.__jobInvoke;
  const context = JSON.parse(globalThis.__jobContext);
  delete globalThis.__jobInvoke;
  delete globalThis.__jobContext;
  const call = (action, input = null) => {
    const reply = JSON.parse(invoke(action, JSON.stringify(input)));
    if (reply.error) throw new Error(reply.error);
    return reply.value;
  };
  return Object.freeze({
    ...context,
    state: Object.freeze({
      load: () => call("state.load"),
      save: (value) => call("state.save", value),
    }),
    metadata: Object.freeze({ query: (query = {}) => call("metadata.query", query) }),
    notes: Object.freeze({
      frontmatter: (path) => call("notes.frontmatter", { path }),
      patchFrontmatter: (patch) => call("notes.patchFrontmatter", patch),
    }),
    notifications: Object.freeze({ notify: (notification) => call("notifications.notify", notification) }),
    signal: Object.freeze({ get aborted() { return call("cancelled"); } }),
  });
})()
