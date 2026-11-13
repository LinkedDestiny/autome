import { type RouteConfig, index, route } from "@react-router/dev/routes";
export default [index("routes/_index.tsx"), route("projects/:id", "routes/projects.$id.tsx")] satisfies RouteConfig;
