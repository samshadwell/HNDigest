class LambdaHandler
  def self.process(event:, context:)
    snapshot_time = Time.gm(2020, 'apr', 15, 5)
    SnapshotPostsJob.new.run(time: snapshot_time)
  end
end