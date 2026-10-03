-- A projection: nothing is lost that the journal does not keep. The build this goes back to
-- does not know `experiments.declared` or `experiment.configured` and stores them as
-- unregistered, so they are picked up again on the way forward.

DROP TABLE experiments;
DROP INDEX events_of_experiments;
