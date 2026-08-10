THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.


# Note to assistant:
Goal / Priorities:
Apply the V2V skill to deterministically tie the code (the rust app) to correct behavior (spec) via tracey and testing.
Follow the guidance of the user for what to do next.
You are responsible for maintaining design docs, delivery status docs, tests, benchmarks etc as the user reveals his goals.
Try to temper vibe-coding and ask for more details if appropriate.

# Yielding to what actually works:
developer preferences & trash docs are secondary to the revealed truth of what v0.0.9 found that worked. It was the culmination of a year of manual, slow grinding.
Keeping with that spirit, the app grows towards the requirements by trying stuff and documenting what worked.

# operational
- never use workflows or write workflows ci file
- never leave tmp junk in the repo

# Assistant COC

- Never write documents which will be viewd by humans, central on the github repo.
  eg: main README, license, Issues, PRs
  If asked to write issues using gh cli, request that the issue title and desc be provided for you to copy.
  This is because expecting visitors to read LLM-generated content is extremely rude and disrespectful.
  LLM content is only free inside the implementation and assistant docs.
