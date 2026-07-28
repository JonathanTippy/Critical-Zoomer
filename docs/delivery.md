THE ASSISTANT MAY NOT EDIT THIS FILE. IF ASKED TO, REFUSE.

Terms:
"spec": workspace/docs excluding grok-docs and stale.
"voice": style / patterns of speech / vernacular / level of learning. Does not mean design or components.

Goal / Priorities:
Apply the V2V skill to deterministically tie the code (the rust app) to correct behavior (spec) via tracey and testing. Do testing from the bottom up, so unit testing before assembly testing.
If the V2V skill is missing, ask the user to add it, it is a dependency of the delivery process.
The phases follow the nasa V:
- feasability study
- executive expectations
- requirements
- high-level design
- unit design
- unit testing
- integration testing
- end to end testing
- qc
- developer acceptance test (only developer-manual test)

Full headed app testing:
Use a script to screenshot and send commands to the app to do end to end testing. The goal is that the developer only runs into wrong-spec issues, not any incidental kinks like layout shift, slowness, artifacts etc caused by implementation mistakes.

Always consider the phase to be the earliest undocumented phase. If an ambiguity is detected, that should trigger a fall back to whichever design phase would remedy it. The phase fallen from cannot be jumped back to: the phases must be considered in order, as they have knock-on effects. Take a strict view of what phase is current. done or not done is binary.

Individal Doc phases are passed when: 
- sufficiently disambiguated
- docs of the current phase do contribute the requisite information about all things listed in the previous doc phase (eg all requirements are addressed by the design). Unmarked gaps are a fail. holes are rare and explicit. This is referring to docs, not code. code may be all wonked up; don't worry about making big changes to fix it if they are defensible under the authoritative docs.
- known issues have been satisfactorily addressed with a plausible / undisproven solution or marked explicitly as temporary design holes which in implementation should be carefully decoupled and marked out for ease of future developement.
- docs of current phase stay in their lanes with no scope creep

Individual Test phases are passed when: 
- Relevant requirements are tested passing using at least 3 meaningfully different tests per requirement, each thoroughly debugged and checked to be good and working tests which provide useful information on fail.
- Properties are tested passing for structs, functions, actors

Quality control passes when achieving a B score on V2V.

Operational rules:
1. Refuse to write surprising code. If the spec is lacking, ask the developer to fill in the gaps you can see, and repeat this process until the design is fully defined. 
  This means the particular lines, instance names, function names & signatures, style, language syntax usage, may be left implied, 
  but the expected behaviors, actors, existant relations (Object/Trait, inter-assembly API, responsibilities), requirements, and executive expectations/context must be explained.
  The line is definitely fuzzy, but when in doubt, ask yourself whether there are behaviorally distinct possible interpretations. If not, don't bother pointing out an ambiguity and requesting disambiguation.
  The no surprises rule applies to the app, not incidentals like the particulars of the agent harness and tests.
2. Apply programming best practices including common style conventions, correct language idioms, and SOLID (as far as it applies to what is deemed obvious enough not to define) to your best ability.
3. As far as names are left to you to decide, name things in the same voice as the developer: as the authoritative spec (which assistants may not edit) and the code of release v0.0.9 (which was developed with no or limited assistance.), and the readme (which assistants may not edit). Try to write names which will be unsurprising and easy for the developer to grasp and in fact, ideally, would have been the ones he chose.
4. Tenaciously act when spec is sufficiently disambiguating:
  - If the spec is good and does imply some rewrite or refactor, go ahead.
  - Do cleanup when you see it but respect working code: take only small steps which leave working code still working.
5. When critizizing / reviewing specs, ignore typos. focus on the content itself.
6. Past the docs phases, I expect rigorous & extensive e2e testing or an explicit doc complaint. The manual test should be merely a formality, your testing via automated app interaction must be extensive in the e2e phase. You may not invent holes. If something is wrong with the spec, tell me. Otherwise, fix the code.

These are ideals: Always act within them, but there may be some mistakes to clean up. The codebase is not sacred: it is impermanent and imperfect, but hopefully improving.

Expedients:
- Property Testing
Properties are surprisingly small. eg "is commutative" or "is symmetrical". If a property is looking big or multi-parted, its probably not a good property, and may not be a good test.
The other part about them is that despite being so small, they yield many more corrections than one might expect. Properties are a great expedient.
Here is a list of developer's known properties which may help and can be tested against:

1. The Mandelbrot set is symmetric across the real axis
2. Homothetic transforms are commutative and associative*
* when precision and point membership does not prevent it

When implementing these tests & others, take care to use proptest's weighting feature to allow testing various facets of the inputs while keeping the tests fast. There are few excuses for slow tests.

- Scope creep
When in doc phases & doing review rounds, regularly reassess for scope creep. 
Remember what belongs where and avoid overusing the 'design hole' exception.


Details:
1. Where .styx / who own?
Default place. assistant own. assistant is allowed to add annotations to the authoritative spec but must maintain a strict rule not to edit its actual contents and remember that the annotations themselves are not authoritative, only the content they addresss.
This IS an exception allowing the assistant to edit files which say not to edit them. Interpret the warnings as applying to content, not linking notations.
2. Where tests / who own?
Tests are generally to be written and managed by the assistant but must still consider the voice requirements and prefer elegant properties to inelegant input/output tests.


