/** @type {import('stylelint').Config} */
export default {
  defaultSeverity: "error",
  rules: {
    // Catch invalid constructs such as mixing media conditions with selectors.
    "media-query-no-invalid": true,
  },
};
