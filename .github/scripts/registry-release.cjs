const registry = { owner: 'Chiv2-Community', repo: 'C2ModRegistry' };

async function prepareRequest(github, repository, tag) {
  if (!/^v\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tag)) {
    throw new Error('Registry requests require a semantic version tag beginning with v.');
  }
  const { data: release } = await github.rest.repos.getReleaseByTag({ ...repository, tag });
  if (release.draft || release.tag_name !== tag) {
    throw new Error('The tagged release must be published before requesting registration.');
  }
  if (!release.assets.some(asset => asset.name === 'UnchainedPlugin.dll' && asset.state === 'uploaded' && asset.size > 0)) {
    throw new Error('The release must contain an uploaded, nonempty UnchainedPlugin.dll.');
  }
  const repoUrl = `https://github.com/${repository.owner}/${repository.repo}`;
  const payload = { action: 'add-dll-release', repo_url: repoUrl, release_tag: tag };
  const marker = `<!-- c2modregistry:${repository.owner}/${repository.repo}:${tag} -->`;
  return {
    title: `[DLL RELEASE] UnchainedPlugin Version ${tag}`,
    body: `${marker}\n\n\`\`\`json\n${JSON.stringify(payload, null, 2)}\n\`\`\`\n`,
    marker,
  };
}

async function submitRequest(github, request) {
  // Include closed issues so rerunning a successful or rejected release never
  // creates another approval request. Failed requests can be retried in place.
  const issues = await github.paginate(github.rest.issues.listForRepo, { ...registry, state: 'all', per_page: 100 });
  const existing = issues.find(issue => !issue.pull_request && issue.body?.includes(request.marker));
  if (existing) return existing;
  const { data: issue } = await github.rest.issues.create({ ...registry, title: request.title, body: request.body });
  return issue;
}

module.exports = { prepareRequest, submitRequest };
