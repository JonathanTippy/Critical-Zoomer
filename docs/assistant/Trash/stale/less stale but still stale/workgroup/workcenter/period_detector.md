There must be no separate 'period detection phase.' 
The period detector runs on all iterated points. 
Points filled out with period bucket fill still must be computed for other results like small time and min magnitude.

Period must be detected by first a loop detector: either tortiois and hare or POT iteratoin count snapshots. Equality must be checked at each iteration to obtain a correct period contender. Anytime a period is thought found, the two z values must undergo a twin test. The twin test iterates the two z values a maximum of const N iterations (configurable for best results). If they were determined to be equal spacially, and their derivatives also, through all of the N tests, they are decided to be twins.

If they are twins, the possible period is determined to be the actual period.
