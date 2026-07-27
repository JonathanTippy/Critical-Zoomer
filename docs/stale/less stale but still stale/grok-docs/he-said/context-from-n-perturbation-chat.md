# PO/dev record — quotes from N-perturbation → Critical-Zoomer `grok-probation` chat

Source: agent transcript `bbe4685d-0c57-4c78-9041-cc4082b33b48` (and follow-ons).  
Unedited quotes preferred. Light headings only for findability. Later quotes may supersede earlier ones (e.g. research vs `grok-probation` quality bar).

---

## Vehicle / quality bar

> Wait, I changed my mind. I want to put this all on a branch in Critical-Zoomer, I've just created it, its called grok-probation. This means the goal will be real app quality, not research. Also, you will be starting from where Critical-Zoomer is currently, lacking any perturbation code. The plan will need refactoring.

Earlier (research-era; superseded for vehicle, still useful historically):

> That is not the design I want and I want you to make a note of it. We are testing perturbation, not building a well-behaved app. The naive worker was only there for us to test out the refactor and associated changes, and shouldn't be used. Besides, the zero case is always extremely important, and the Z=0 reference orbit serves the role of the naive method perfectly, in theory.

> Also, yes, we are working in the current repo. Once again, very silly question.

---

## Proximate remap

> You have poked a hole in the tire and declared the bumpiness is fixed! (but its only fixed because the car can no longer drive).
> Low-res remap is a pivotal part of the base identity of the program and its value as a real time viewer. It is not a preview and must never be called one: it is a proximate intermediate which fills the gap between what the user has required the program to display and what information the program actually has. The app is not allowed to, must not show the user a flat empty pane when it has the opportunity to remap the previous work.
> Pause work for now. Make absolutely certain you will not make this mistake again via the docs and a skill.
> I do need to justify why I am guarding this app-type behavior even though this is a research prototype. The reason is that this feature is pivotal to being able to properly navigate manually.

> dont forget the main thing I was referring to is you broke remap *again*

---

## Classical perturbation — do not cheat

> 1. falling back to naive is cheating (unless you mean perturbtaion reference orbit 0 which is valid)
> 2. maximum iteration count is banned (unless you are referring to some preliminary or temporary limit)
> 3. I can see in the screenshots that the reference orbits being chosen are not in the set. reference orbits must be in the set.
> 4. If the periodicity detection is correct, "never finishing seats" will not exist, only very slow finishing ones. Again, temporarily pausing a very slow point is ok, but giving up is banned.
> 5. Classical Perturbation rests on very strong orbit comprehension, the cornerstone of which is correct periodicity and period detection.
> 6. you have still not demonstrated any precision past z=17 which would prove you are even using perturbation.
> Consider all these things. Tell me if you disagree with any.

> Make notes to ensure these points are persisted, and address all in the codebase. Obtain  a complete and correct and working classical perturbation implementation which demonstrates real precision gain and does not cheat.

> Remember that falling back to naive is not allowed (unless you meant the 0 case)

> you just did the same mistake again. fix it and fix perturbation. no cheating by scaffolding in unnecessary values. Remember that there are only two precision levels and one is (designally) way too large to contend with hardly at all at working timescale, even though atm its f64.
> Stop assuming the HUD output speaks for the view content. it is often just a grey panel and you claim its ok.

> Also remember that early on, points can and should fall back to the trivial 0-orbit if an orbit isn't ready yet.

---

## Zero orbit / last orbit / no composer if-forests

> 1. The zero-orbit should *definitely* be used for compute if precisions allow It is NOT a special case and I can't stand how all of the code composer wrote contains unnecesary if statements instead of just writing the code in such a way that the zero case works and then using the zero case. It appears to be a systemic stupidity of composer-2.5, so please inspect the recent changes for more instances of it. My designs were loosely coupled and elegant, but composer produced what can only be described as slop.
> 2. yes, the last orbit should be kept, that is fine for now.

> New problem: reference orbits were made with IntExp definitions for good reason: they are intended to be freely reused across levels of magnification. There must not be a "perturbatoin pause", a period of time where no visual progress is made because the reference orbit is being computed. Obvisouly this is sometimes unavoiadable if there is actually not a workable reference available, but the way the app behaves in manual testing implies that it is not even trying. Report the current behavior.

---

## Per-seat orbits

> 1. each seat references its own orbit. session-wide reference management is simply retarded.
> 2. I think that sounds right, I'm not a big GPU guy though.

(Context for #2: wgpu as GPU API.)

---

## Lookahead, collection, tiles, types, GPU (planning dump)

> Along with that cleanup, there is more: examine the type/trait situation compared with my WIP rewrite in ~/git/Critical-Zoomer. Is Mandelbrotable used for all screenspace math? Is there a comparable trait for reference orbit computation?
> One other thing, the black rectangle on the upper left is the coordinate input. Find a better spot to put it so 1. it doesnt cover up fps and 2. its not black, which is a confusing color.
> After the cleanup and verifying no regressions, the next steps:
> 1. lookahead reference orbit computation: the reference manager should always seek to compute references near the mouse cursor which would be useful if the user were to zoom in. Besides that, it should maintain a collection of references, intelligently keeping it within a memory budget, but many references are often necessary in glitchy areas.
> 2. double check reference orbit computation is as fast as it should be: the pauses I was just seeing are not impressive. Biggest win is keeping everything small and on stack. Also, channel health is not looking good. Consult the latest design in ~/git/Critical-Zoomer and implement the entire tiles + GPU API rewrite.
> 3. Add new small intexp type: [i64] and a i32. const type parameter 'stacks' defines how many 64 bit parts the intexp will have.
> The type must be on stack but must support large precision via an array. Addition/multiplication can be propagated via i128. Also ensure reference computation can use rug float, as this will likely be faster for more precise values.
> Also construct a floatexp out of rug float + i32. This will not be necessary right away, as it is too stacky for screenspace, but will be useful for N-perturbation.
> Also construct a floatexp type with f64 and f32.
> Perturbation requires a multiplier term to zoom deep, but I intend to sidestep the issue by constructing depth-capable types. Its pretty simple, it just takes an i32 exponent and then the range is bonkers.
> 4. Also write the GPU worker, at this point it will work on tiles. Tiles are very cool, they bridge the seral/parallel gap nicely, more explained in the docs in ~/git/Critical-Zoomer. For compat, it can only use 32 bit, so it will be limited to f32, f32 + i32, and i32 + i32 (if you can work out how to capture the overflow).
>
> The overarching goal is that this new perturbation code is neat, well designed, and extremely fast, while guarding against regressions. Ask design questions now, and don't stop when you are applying the plan.

---

## Tiles / assembly

> explain tiles questoin in more detail. Should be explained in /home/jonathan/git/Critical-Zoomer/docs/architecture.md .
> worker uses tiles which are CPU. for gpu work, the batch (N points) is uploaded to the GPU during initialization and results are downloaded as they come out. Tiles are then sent through the GPU uploader actor into the headgroup hoard manager which samples the tiles and shades the sampled answer frame, all at framerate. The design is much simplere than before and should also be much faster.
> Incidentally, this does change the worker to work chiefly on tiles, not screens. This also makes comprehension of the working area possible, as if its were the whole screen it would be way too mcuh data to remap quickly enough. Also, working one tile at a time ensures rendering is more predictable and steerable.
> Did that answer the questoin or just raise more?

---

## IntExp stays; StackedIntExp additive only

> I don't like the langauge of "migrate references/deep first" in your previous response. It is inteded solely for calculating reference orbits and perhaps screen computations as needed. IntExp is part of the essential skeleton on the app and getting rid of it owuld be silly so I'm disappointed in you for even suggesting it.
> Also, yes, we are working in the current repo. Once again, very silly question.

---

## Process

> Sounds good. per-phase questions are back in: just make sure they are batched at the end or beginning of the phases so the process isn't constatnly stalling becuase I have to get around to answering a question.

> One other thing, the screenshot / app usage script from N-perturbation came in handy for you to prevent regressions, your first step should be bringing it over, but if you like, redesign it, as you're smarter than composer, and composer wrote it.
> Anyway, ask the first batch of questions.

> Without implementing the plan, update he-said with everything I've shared in this context.

> Oh, one other thing for the he-said area, which I also added to the readme: For best accuracy, value direct, unedited quotes highly. Aim for most of the content in this are to follow that form.

---

## Batch A answers

> 1. It must be defined by a const in constants.rs, called tile edge length pot. set it to 6 (64), but use it in all code so that it can be trivially changed to other nearby values. I don't expect to change it but owuld like the option.
> 2. reuse Answer: not really reuse, I wrote it and I like it, its good. but like anything, it may need changes. In fact, I already know that it does need at least one: edge detection is changing to derivative direction storage in answers, so that edge detection can be done outside the workgroup. Escape time change direction will need to be computed via derivated and added to answers. For the headgroup to recieve, You will probably need to create a gpu variant, which does raise questions about how to fit things into 32 bits.
> 3. explain your question better it is too ambiguous.

> A is good as long as the adapter is explicitly temporary

---

## Scope / wheels (research debugging era)

> Get back to the perturbation issues. Remember that the wheels are not in question. Focus and scope your thinking to the perturbation implementation and reference manager.
> You are debugging the main worker working on the current stencil, not anything else.
> It will greatly simplify your thinking to scope it to the workcore only.
> Also, the corrupted results have returned again. Check screenshots to reproduce it and fix it.
> Then fix the precision wall at ~17. The goal is that perturbation uses f32 yet inherits precision limit from the f64 orbit (which is stored in f32), as a proof of concept / prototype.
> Test it with your screenshot tool very thoroughly until it is behaving properly.
