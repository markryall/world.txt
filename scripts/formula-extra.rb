  def caveats
    <<~EOS
      For sound in the terminal client (radios, gramophones, jukeboxes, hums
      and the island's own sounds), and to play your own songs from it:
        brew install mpv
      Then type `sound on` in world.txt play.
    EOS
  end

  test do
    assert_match "world.txt #{version}", shell_output("#{bin}/world.txt --version")
  end
