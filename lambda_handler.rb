# frozen_string_literal: true

require_relative 'jobs/snapshot_posts_job'

# 5 AM UTC -> 10pm PDT, 9pm PST
SNAPSHOT_DAILY_HOUR = 5

def handle(*)
  current_time = Time.now
  snapshot_time = Time.gm(
    current_time.year,
    current_time.month,
    current_time.day,
    SNAPSHOT_DAILY_HOUR
  )
  SnapshotPostsJob.new.run(time: snapshot_time)
end
