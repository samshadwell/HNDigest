# frozen_string_literal: true

require_relative 'jobs/snapshot_posts_job'

def handle(*)
  snapshot_time = Time.gm(2020, 'apr', 15, 5)
  SnapshotPostsJob.new.run(time: snapshot_time)
end
